use std::{
    collections::{HashMap, HashSet}, ffi::{c_void, CString}, fmt::{Display, Debug}, future::Future, ops::{Deref, DerefMut}, sync::LazyLock, task::{Poll, Waker}
};

use color_eyre::eyre::Context;
use libmpv::{events::{Event, EventContext}, LogLevel, Mpv};
use libmpv_sys::{
    mpv_format_MPV_FORMAT_NODE_MAP, mpv_format_MPV_FORMAT_STRING,
    mpv_log_level_MPV_LOG_LEVEL_DEBUG, mpv_log_level_MPV_LOG_LEVEL_ERROR,
    mpv_log_level_MPV_LOG_LEVEL_FATAL, mpv_log_level_MPV_LOG_LEVEL_INFO,
    mpv_log_level_MPV_LOG_LEVEL_TRACE, mpv_log_level_MPV_LOG_LEVEL_V,
    mpv_log_level_MPV_LOG_LEVEL_WARN, mpv_node, mpv_node__bindgen_ty_1, mpv_node_list,
    mpv_set_property, mpv_set_wakeup_callback,
};
use parking_lot::{Mutex, RwLock};
use reqwest::header::{HeaderName, HeaderValue};
use tracing::{
    field::FieldSet, info, level_enabled, level_filters::STATIC_MAX_LEVEL, Level, Metadata,
};
use tracing_core::{callsite::DefaultCallsite, identify_callsite, Callsite, LevelFilter};

pub struct AsyncMpv {
    inner: Mpv,
    waker: Box<Mutex<Option<Waker>>>,
}

impl Deref for AsyncMpv {
    type Target = Mpv;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub unsafe fn wake(waker_ptr: *const Mutex<Option<Waker>>) {
    let waker = &*waker_ptr;
    if let Some(waker) = waker.lock().deref_mut() {
        waker.wake_by_ref();
    }
}

unsafe extern "C" fn wake_callback(cx: *mut c_void) {
    wake(cx.cast_const().cast());
}

pub struct EventFuture<'mpv> {
    event_context: Option<&'mpv mut EventContext<'mpv>>,
    waker: &'mpv Mutex<Option<Waker>>,
}



impl<'mpv> Future for EventFuture<'mpv> {
    type Output = Result<libmpv::events::Event<'mpv>, MpvError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        *self.waker.lock() = Some(cx.waker().clone());
        match self.get_mut().event_context.take().unwrap().wait_event(0.0) {
            Some(Ok(e)) => Poll::Ready(Ok(e)),
            Some(Err(e)) => Poll::Ready(Err(e.into())),
            None => Poll::Pending,
        }
    }
}

struct MpvError{
    inner: libmpv::Error
}

impl Display for MpvError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Debug for MpvError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl From<libmpv::Error> for MpvError{
    fn from(value: libmpv::Error) -> Self {
        Self { inner: value }
    }
}

impl std::error::Error for MpvError{}

impl AsyncMpv {
    pub fn new(inner: Mpv) -> Self {
        let waker = Box::new(Mutex::new(None));
        let waker_ptr: *const _ = waker.as_ref();
        unsafe {
            mpv_set_wakeup_callback(
                inner.ctx.as_ptr(),
                Some(wake_callback),
                waker_ptr.cast_mut().cast(),
            );
        }
        Self { inner, waker }
    }

    fn wait_event_async<'a>(
        &'a self,
        event_context: &'a mut EventContext<'a>,
    ) -> EventFuture<'a> {
        EventFuture {
            event_context: Some(event_context),
            waker: self.waker.as_ref(),
        }
    }
}

async fn run_episode(mpv: &mut AsyncMpv, event_context: &mut EventContext<'_>)->crate::Result<bool>{

    loop{
        match mpv.wait_event_async(event_context).await.context("waiting for mpv events"){
            Ok(Event::LogMessage { prefix, level:_, text, log_level })=> log_message(prefix, log_level, text),
            Err(e) => return Err(e),
        }
    }

    Ok(true)
}

pub trait MpvExt {
    fn set_header(&self, name: &HeaderName, value: &HeaderValue) -> Result<(), libmpv::Error>;
}

impl MpvExt for Mpv {
    fn set_header(&self, name: &HeaderName, value: &HeaderValue) -> Result<(), libmpv::Error> {
        let name = CString::new(name.as_str())?;
        let value = CString::new(value.as_bytes())?;
        let option = c"http-header-fields";
        let mut value_node = mpv_node {
            u: mpv_node__bindgen_ty_1 {
                string: value.as_ptr().cast_mut(),
            },
            format: mpv_format_MPV_FORMAT_STRING,
        };
        let mut key_list = [name.as_ptr().cast_mut()];
        let mut node_list = mpv_node_list {
            num: 1,
            values: &mut value_node,
            keys: key_list.as_mut_ptr(),
        };
        unsafe {
            let res = mpv_set_property(
                self.ctx.as_ptr(),
                option.as_ptr(),
                mpv_format_MPV_FORMAT_NODE_MAP,
                (&mut node_list as *mut mpv_node_list).cast(),
            );
            if res < 0 {
                Err(res.into())
            } else {
                Ok(())
            }
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
