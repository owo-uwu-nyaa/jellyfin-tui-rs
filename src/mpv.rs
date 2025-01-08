use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
    sync::LazyLock,
};

use color_eyre::eyre::{Context, Result};
use libmpv::{events::Event, LogLevel, Mpv};
use libmpv_sys::{
    mpv_log_level_MPV_LOG_LEVEL_DEBUG, mpv_log_level_MPV_LOG_LEVEL_ERROR,
    mpv_log_level_MPV_LOG_LEVEL_FATAL, mpv_log_level_MPV_LOG_LEVEL_INFO,
    mpv_log_level_MPV_LOG_LEVEL_TRACE, mpv_log_level_MPV_LOG_LEVEL_V,
    mpv_log_level_MPV_LOG_LEVEL_WARN,
};
use parking_lot::RwLock;
use reqwest::header::{HeaderName, HeaderValue};
use tracing::{field::FieldSet, level_filters::STATIC_MAX_LEVEL, Level, Metadata};
use tracing_core::{callsite::DefaultCallsite, identify_callsite, Callsite, LevelFilter};

use crate::TuiContext;

struct MpvPlayer {
    inner: Mpv,
}

impl MpvPlayer {
    pub fn new(cx: &TuiContext) -> Result<Self> {
        let mpv = Mpv::with_initializer(|mpv| Ok(()))?;
        Ok(Self { inner: mpv })
    }
}

async fn run_episode(mpv: &mut Mpv) -> crate::Result<bool> {
    loop {
        match mpv
            .wait_event_async()
            .await
            .context("waiting for mpv events")
        {
            Ok(Event::LogMessage {
                prefix,
                level: _,
                text,
                log_level,
            }) => log_message(prefix, log_level, text),
            Ok(Event::Shutdown) => return Ok(false),
            Ok(_) => todo!(),
            Err(e) => return Err(e),
        }
    }

    Ok(true)
}

pub trait MpvExt {
    fn set_header(&self, name: &HeaderName, value: &HeaderValue) -> Result<(), color_eyre::Report>;
}

impl MpvExt for Mpv {
    fn set_header(&self, name: &HeaderName, value: &HeaderValue) -> Result<(), color_eyre::Report> {
        self.set_property("http-header-fields", &[(name.as_str(), value.to_str()?)])?;
        Ok(())
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
