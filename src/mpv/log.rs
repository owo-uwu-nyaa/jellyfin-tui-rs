use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use libmpv::LogLevel;
use libmpv_sys::{
    mpv_log_level_MPV_LOG_LEVEL_DEBUG, mpv_log_level_MPV_LOG_LEVEL_ERROR,
    mpv_log_level_MPV_LOG_LEVEL_FATAL, mpv_log_level_MPV_LOG_LEVEL_INFO,
    mpv_log_level_MPV_LOG_LEVEL_TRACE, mpv_log_level_MPV_LOG_LEVEL_V,
    mpv_log_level_MPV_LOG_LEVEL_WARN,
};
use parking_lot::RwLock;
use tracing::{field::FieldSet, level_filters::STATIC_MAX_LEVEL, Level, Metadata};
use tracing_core::{callsite::DefaultCallsite, identify_callsite, Callsite, LevelFilter};

pub fn log_message(prefix: &str, level: LogLevel, text: &str) {
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
                    Some(&text.trim().to_string() as &dyn tracing::Value),
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
        return metadata;
    }
    let prefix: &'static str = 'prefix: {
        // ensures that lock guard is dropped before writing
        if let Some(prefix) = STATIC_STRING.read().get(prefix) {
            break 'prefix prefix;
        }
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
    let callsite: &'static DefaultCallsite = Box::leak(Box::new(DefaultCallsite::new(metadata)));
    STATIC_CALLSITE.write().insert((prefix, level), callsite);
    callsite
}
