mod fetch_items;

use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    sync::LazyLock,
    time::Duration,
};

use color_eyre::eyre::{Context, Result};
use fetch_items::fetch_items;
use jellyfin::items::MediaItem;
use libmpv::{
    events::{Event, EventContextAsync, EventContextAsyncExt, PropertyData},
    node::{BorrowingCPtr, BorrowingMpvNodeMap, ToNode},
    LogLevel, Mpv,
};
use libmpv_sys::{
    mpv_log_level_MPV_LOG_LEVEL_DEBUG, mpv_log_level_MPV_LOG_LEVEL_ERROR,
    mpv_log_level_MPV_LOG_LEVEL_FATAL, mpv_log_level_MPV_LOG_LEVEL_INFO,
    mpv_log_level_MPV_LOG_LEVEL_TRACE, mpv_log_level_MPV_LOG_LEVEL_V,
    mpv_log_level_MPV_LOG_LEVEL_WARN,
};
use parking_lot::RwLock;
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Padding, Paragraph},
};
use tokio::time::Interval;
use tracing::{field::FieldSet, level_filters::STATIC_MAX_LEVEL, Level, Metadata};
use tracing_core::{callsite::DefaultCallsite, identify_callsite, Callsite, LevelFilter};

use crate::TuiContext;

pub struct MpvPlayer {
    inner: Mpv<EventContextAsync>,
    interval: Interval,
    position: i64,
    current_item: String,
}

impl MpvPlayer {
    pub async fn new(cx: &TuiContext, item: MediaItem) -> Result<Self> {
        let (items, initial_position) = fetch_items(cx, item).await?;
        let mpv = Mpv::with_initializer(|mpv| -> Result<()> {
            mpv.set_property(c"ytdl", false)?;
            mpv.set_property(c"title", c"jellyfin-tui-player")?;
            mpv.set_property(c"fullscreen", true)?;
            mpv.set_property(c"drag-and-drop", false)?;
            mpv.set_property(c"osc", true)?;
            mpv.set_property(c"terminal", false)?;
            mpv.set_property(
                c"http-header-fields",
                &BorrowingMpvNodeMap::new(
                    &[BorrowingCPtr::new(c"authorization")],
                    &[CString::new(cx.jellyfin.get_auth().header.as_bytes())
                        .context("converting auth header to cstr")?
                        .to_node()],
                ),
            )?;
            mpv.set_property(c"input-default-bindings", true)?;
            mpv.set_property(c"input-vo-keyboard", true)?;
            mpv.set_property(c"idle", c"yes")?;
            mpv.set_property(
                c"hwdec",
                CString::new(cx.config.hwdec.as_str())
                    .context("converting hwdec to cstr")?
                    .as_c_str(),
            )?;
            Ok(())
        })?
        .enable_async();
        mpv.playlist_replace(
            &CString::new(cx.jellyfin.get_video_url(&items[initial_position]))
                .context("converting video url to cstr")?,
        )
        .context("adding to playlist")?;
        for item in items[0..initial_position].iter().rev() {
            mpv.playlist_insert_at(
                &CString::new(cx.jellyfin.get_video_url(item))
                    .context("converting video url to cstr")?,
                0,
            )?;
        }
        for item in items[initial_position + 1..].iter() {
            mpv.playlist_append(
                &CString::new(cx.jellyfin.get_video_url(item))
                    .context("converting video url to cstr")?,
            )?;
        }

        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        Ok(Self {
            inner: mpv,
            interval,
            position: 0,
        })
    }

    async fn run_episode(
        &mut self,
        cx: &mut TuiContext,
        title: &str,
        subtitle: Option<&str>,
        id: &str,
    ) -> Result<bool> {
        self.position = 0;
        let title = Paragraph::new(title).centered();
        let subtitle = subtitle.map(|subtitle| Paragraph::new(subtitle).centered());
        let block = Block::bordered()
            .title("Now playing")
            .padding(Padding::uniform(1));
        loop {
            cx.term.draw(|frame| {
                frame.render_widget(&block, frame.area());
                let area = block.inner(frame.area());
                if let Some(subtitle) = &subtitle {
                    let [title_a, subtitle_a] =
                        Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)])
                            .vertical_margin(3)
                            .areas(area);
                    frame.render_widget(&title, title_a);
                    frame.render_widget(subtitle, subtitle_a);
                } else {
                    frame.render_widget(&title, area);
                }
            })?;
        }

        Ok(true)
    }

    async fn recv_mpv_events(&mut self) -> Result<bool> {
        loop {
            tokio::select! {
                _ = self.interval.tick() => {}
                event = self.inner
                    .wait_event_async() => {
                        match event.context("waiting for mpv events")
                        {
                            Ok(Event::LogMessage {
                                prefix,
                                level: _,
                                text,
                                log_level,
                            }) => log_message(prefix, log_level, text),
                            Ok(Event::Shutdown) => break Ok(false),
                            Ok(Event::Idle) => break Ok(true),
                            Ok(Event::CommandReply {
                                reply_userdata: _,
                                data: _,
                            }) => {}
                            Ok(Event::GetPropertyReply {
                                name,
                                result,
                                reply_userdata,
                            }) => {
                                match (name, reply_userdata){
                                    ("time-pos", 6969) => {
                                        if let PropertyData::Int64(position) = result{
                                            self.position = position;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Ok(_) => {}
                            Err(e) => break Err(e),
                        }
                        continue;
                    }
            }
            self.inner.get_property_async::<i64>("time-pos", 6969)?;
        }
    }
}
fn log_message(prefix: &str, level: LogLevel, text: &str) {
    #[allow(non_upper_case_globals)]
    let level = match level {
        mpv_log_level_MPV_LOG_LEVEL_FATAL | mpv_log_level_MPV_LOG_LEVEL_ERROR => Level::ERROR,
        mpv_log_level_MPV_LOG_LEVEL_WARN => Level::WARN,
        mpv_log_level_MPV_LOG_LEVEL_INFO => Level::INFO,
        mpv_log_level_MPV_LOG_LEVEL_V | mpv_log_level_MPV_LOG_LEVEL_DEBUG => Level::DEBUG,
        mpv_log_level_MPV_LOG_LEVEL_TRACE => Level::TRACE,
        level => panic!("Unknown mpv log level: {level}"),
    };
    if level <= STATIC_MAX_LEVEL && level <= LevelFilter::current() {
        let callsite = get_tracing_callsite(prefix, level);
        let interest = callsite.interest();
        let metadata = callsite.metadata();
        if !interest.is_never()
            && (interest.is_always() || tracing::dispatcher::get_default(|d| d.enabled(metadata)))
        {
            let fields = metadata.fields();
            tracing::Event::dispatch(
                metadata,
                &fields.value_set(&[(
                    &fields.iter().next().unwrap(),
                    Some(&text.to_string() as &dyn tracing::Value),
                )]),
            );
        }
    }
}

static STATIC_STRING: LazyLock<RwLock<HashSet<&'static str>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

static STATIC_CALLSITE: LazyLock<RwLock<HashMap<(&'static str, Level), &'static DefaultCallsite>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn get_tracing_callsite(prefix: &str, level: Level) -> &'static DefaultCallsite {
    if let Some(metadata) = STATIC_CALLSITE.read().get(&(prefix, level)) {
        metadata
    } else {
        let prefix: &'static str = if let Some(prefix) = STATIC_STRING.read().get(prefix) {
            prefix
        } else {
            let prefix = prefix.to_string().leak();
            STATIC_STRING.write().insert(prefix);
            prefix
        };
        static MESSAGE_FIELD: &[&str] = &["message"];
        static MESSAGE_FIELD_SET_CALLSITE: DefaultCallsite = DefaultCallsite::new({
            static META: Metadata = Metadata::new(
                "empty_field_set",
                "this is stupid",
                Level::ERROR,
                None,
                None,
                None,
                FieldSet::new(
                    MESSAGE_FIELD,
                    identify_callsite!(&MESSAGE_FIELD_SET_CALLSITE),
                ),
                tracing_core::Kind::EVENT,
            );
            &META
        });
        let metadata: &'static Metadata<'static> = Box::leak(Box::new(Metadata::new(
            "libmpv log message",
            prefix,
            level,
            None,
            None,
            None,
            FieldSet::new(
                MESSAGE_FIELD,
                identify_callsite!(&MESSAGE_FIELD_SET_CALLSITE),
            ),
            tracing_core::Kind::EVENT,
        )));
        let callsite: &'static DefaultCallsite =
            Box::leak(Box::new(DefaultCallsite::new(metadata)));
        STATIC_CALLSITE.write().insert((prefix, level), callsite);
        callsite
    }
}
