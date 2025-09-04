use std::{
    ffi::CString,
    ops::Deref,
    task::{Poll, ready},
};

use color_eyre::eyre::{Context, Result};
use futures_util::Stream;
use jellyfin::JellyfinClient;
use libmpv::{
    Format, Mpv,
    events::{
        Event, EventContextAsync, EventContextAsyncExt, EventContextExt, PropertyData, mpv_event_id,
    },
    node::{BorrowingMpvNodeList, ToNode},
};
use tracing::{info, instrument, trace, warn};

use crate::Config;

use super::log::log_message;

#[derive(Debug)]
pub enum ObservedProperty {
    Position(f64),
    Idle(bool),
    Pause(bool),
}

#[derive(Debug)]
pub enum MpvEvent {
    PropertyChanged(ObservedProperty),
    StartFile(i64),
}

pub struct MpvStream {
    mpv: Mpv<EventContextAsync>,
}

impl Deref for MpvStream {
    type Target = Mpv<EventContextAsync>;
    fn deref(&self) -> &Self::Target {
        &self.mpv
    }
}

impl Stream for MpvStream {
    type Item = Result<MpvEvent>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        Poll::Ready(loop {
            let event = match ready!(self.mpv.poll_wait_event(cx)).context("waiting for mpv events")
            {
                Err(e) => break Some(Err(e)),
                Ok(v) => v,
            };
            trace!(?event);
            match event {
                Event::LogMessage {
                    prefix,
                    level: _,
                    text,
                    log_level,
                } => log_message(prefix, log_level, text),
                Event::Shutdown => break None,
                Event::StartFile { playlist_entry_id } => {
                    break Some(Ok(MpvEvent::StartFile(playlist_entry_id)));
                }
                Event::PropertyChange {
                    name,
                    change,
                    reply_userdata,
                } => match (name, change, reply_userdata) {
                    ("time-pos", PropertyData::Double(pos), 1) => {
                        break Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Position(
                            pos,
                        ))));
                    }
                    ("idle-active", PropertyData::Flag(idle), 2) => {
                        break Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Idle(idle))));
                    }
                    ("pause", PropertyData::Flag(pause), 3) => {
                        break Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Pause(
                            pause,
                        ))));
                    }
                    (name, val, id) => {
                        warn!(name, ?val, id, "received unrequested property change event");
                    }
                },
                _ => {}
            }
        })
    }
}

impl MpvStream {
    #[instrument(skip_all)]
    pub fn new(jellyfin: &JellyfinClient, config: &Config) -> Result<Self> {
        let mpv = Mpv::with_initializer(|mpv| -> Result<()> {
            mpv.set_option(c"title", c"jellyfin-tui-player")?;
            mpv.set_option(c"fullscreen", true)?;
            mpv.set_option(c"drag-and-drop", false)?;
            mpv.set_option(c"osc", true)?;
            mpv.set_option(c"vo", c"gpu-next")?;
            mpv.set_option(c"terminal", false)?;
            let mut header = b"authorization: ".to_vec();
            header.extend_from_slice(jellyfin.get_auth().header.as_bytes());
            mpv.set_option(
                c"http-header-fields",
                &BorrowingMpvNodeList::new(&[CString::new(header)
                    .context("converting auth header to cstr")?
                    .to_node()]),
            )?;
            mpv.set_option(c"input-default-bindings", true)?;
            mpv.set_option(c"input-vo-keyboard", true)?;
            mpv.set_option(
                c"hwdec",
                CString::new(config.hwdec.as_str())
                    .context("converting hwdec to cstr")?
                    .as_c_str(),
            )?;
            config.mpv_profile.initialize(&mpv)?;
            Ok(())
        })?
        .enable_async();
        mpv.set_log_level(
            &CString::new(config.mpv_log_level.clone()).context("converting log level to cstr")?,
        )?;
        mpv.enable_event(mpv_event_id::PropertyChange)?;
        mpv.enable_event(mpv_event_id::LogMessage)?;
        mpv.enable_event(mpv_event_id::QueueOverflow)?;
        mpv.enable_event(mpv_event_id::StartFile)?;
        mpv.observe_property("time-pos", Format::Double, 1)?;
        mpv.observe_property("idle-active", Format::Flag, 2)?;
        mpv.observe_property("pause", Format::Flag, 3)?;
        info!("mpv initialized");
        Ok(Self { mpv })
    }
}
