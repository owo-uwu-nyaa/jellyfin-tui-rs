use std::{
    ffi::{c_void, CString},
    future::Future,
    ops::{Deref, DerefMut},
    task::{Poll, Waker},
};

use libmpv::{events::EventContext, Mpv};
use libmpv_sys::{
    mpv_format_MPV_FORMAT_NODE_MAP, mpv_format_MPV_FORMAT_STRING, mpv_node, mpv_node__bindgen_ty_1,
    mpv_node_list, mpv_set_property, mpv_set_wakeup_callback,
};
use parking_lot::Mutex;
use reqwest::header::{HeaderName, HeaderValue};

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
    type Output = libmpv::Result<libmpv::events::Event<'mpv>>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        *self.waker.lock() = Some(cx.waker().clone());
        match self.get_mut().event_context.take().unwrap().wait_event(0.0) {
            Some(v) => Poll::Ready(v),
            None => Poll::Pending,
        }
    }
}

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

    pub fn wait_event_async<'a>(
        &'a self,
        event_context: &'a mut EventContext<'a>,
    ) -> EventFuture<'a> {
        EventFuture {
            event_context: Some(event_context),
            waker: self.waker.as_ref(),
        }
    }
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
