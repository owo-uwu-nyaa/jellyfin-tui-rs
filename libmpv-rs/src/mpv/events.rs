// Copyright (C) 2016  ParadoxSpiral
//
// This file is part of libmpv-rs.
//
// This library is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 2.1 of the License, or (at your option) any later version.
//
// This library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public
// License along with this library; if not, write to the Free Software
// Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA

use libmpv_sys::mpv_event;
use mpv::MpvDropHandle;
use protocol::ProtocolContextType;

use crate::{mpv::mpv_err, *};

use std::ffi::{CString, c_void};
use std::future::{Future, pending, poll_fn};
use std::marker::PhantomPinned;
use std::os::raw as ctype;
use std::pin::Pin;
use std::process::abort;
use std::ptr::NonNull;
use std::slice;
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};

/// An `Event`'s ID.
pub use libmpv_sys::mpv_event_id as EventId;

use super::node::MpvNode;
pub mod mpv_event_id {
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_AUDIO_RECONFIG as AudioReconfig;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_CLIENT_MESSAGE as ClientMessage;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_COMMAND_REPLY as CommandReply;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_END_FILE as EndFile;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_FILE_LOADED as FileLoaded;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_GET_PROPERTY_REPLY as GetPropertyReply;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_HOOK as Hook;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_IDLE as Idle;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_LOG_MESSAGE as LogMessage;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_NONE as None;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_PLAYBACK_RESTART as PlaybackRestart;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_PROPERTY_CHANGE as PropertyChange;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_QUEUE_OVERFLOW as QueueOverflow;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_SEEK as Seek;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_SET_PROPERTY_REPLY as SetPropertyReply;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_SHUTDOWN as Shutdown;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_START_FILE as StartFile;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_TICK as Tick;
    pub use libmpv_sys::mpv_event_id_MPV_EVENT_VIDEO_RECONFIG as VideoReconfig;
}

struct PanicAbort;
impl Drop for PanicAbort {
    fn drop(&mut self) {
        eprintln!("waking waker paniced");
        abort()
    }
}

pub struct WakerContext {
    location: Pin<Box<WakerLocation>>,
    interval: interval::DefaultInterval,
}

pub struct WakerLocation {
    inner: Mutex<Option<Waker>>,
    _pin: PhantomPinned,
}

#[cfg_attr(feature = "tracing", tracing::instrument(level = "trace"))]
pub(crate) unsafe extern "C" fn wake_callback(cx: *mut c_void) {
    let abort_guard = PanicAbort;
    #[cfg(feature = "tracing")]
    {
        tracing::trace!("wake_callback called");
    }
    let waker = unsafe { &*cx.cast_const().cast::<WakerLocation>() };
    if let Ok(waker) = waker.inner.lock() {
        if let Some(waker) = &*waker {
            waker.wake_by_ref();
        }
    } else {
        eprintln!("waker poisoned");
        abort()
    }
    std::mem::forget(abort_guard);
}

#[derive(Debug)]
/// Data that is returned by both `GetPropertyReply` and `PropertyChange` events.
pub enum PropertyData<'a> {
    Str(&'a str),
    OsdStr(&'a str),
    Flag(bool),
    Int64(i64),
    Double(ctype::c_double),
    Node(&'a MpvNode),
}

impl<'a> PropertyData<'a> {
    // SAFETY: meant to extract the data from an event property. See `mpv_event_property` in
    // `client.h`
    unsafe fn from_raw(format: MpvFormat, ptr: *mut ctype::c_void) -> Result<PropertyData<'a>> {
        assert!(!ptr.is_null());
        unsafe {
            match format {
                mpv_format::Flag => Ok(PropertyData::Flag(*(ptr as *mut bool))),
                mpv_format::String => {
                    let char_ptr = *(ptr as *mut *mut ctype::c_char);
                    Ok(PropertyData::Str(mpv_cstr_to_str!(char_ptr)?))
                }
                mpv_format::OsdString => {
                    let char_ptr = *(ptr as *mut *mut ctype::c_char);
                    Ok(PropertyData::OsdStr(mpv_cstr_to_str!(char_ptr)?))
                }
                mpv_format::Double => Ok(PropertyData::Double(*(ptr as *mut f64))),
                mpv_format::Int64 => Ok(PropertyData::Int64(*(ptr as *mut i64))),
                mpv_format::Node => Ok(PropertyData::Node(&*(ptr as *mut MpvNode))),
                mpv_format::None => unreachable!(),
                _ => unimplemented!(),
            }
        }
    }
}

#[derive(Debug)]
pub enum Event<'a> {
    /// Received when the player is shutting down
    Shutdown,
    /// *Has not been tested*, received when explicitly asked to MPV
    LogMessage {
        prefix: &'a str,
        level: &'a str,
        text: &'a str,
        log_level: LogLevel,
    },
    /// Received when using get_property_async
    GetPropertyReply {
        name: &'a str,
        result: PropertyData<'a>,
        reply_userdata: u64,
    },
    /// Received when using set_property_async
    SetPropertyReply(u64),
    /// Received when using command_async
    CommandReply {
        reply_userdata: u64,
        data: MpvNode,
    },
    /// Event received when a new file is playing
    StartFile {
        playlist_entry_id: i64,
    },
    /// Event received when the file being played currently has stopped, for an error or not
    EndFile(EndFileReason),
    /// Event received when a file has been *loaded*, but has not been started
    FileLoaded,
    ClientMessage(Vec<&'a str>),
    VideoReconfig,
    AudioReconfig,
    /// The player changed current position
    Seek,
    PlaybackRestart,
    /// Received when used with observe_property
    PropertyChange {
        name: &'a str,
        change: PropertyData<'a>,
        reply_userdata: u64,
    },
    /// Received when the Event Queue is full
    QueueOverflow,
    Idle,
    /// A deprecated event
    Deprecated(mpv_event),
}

pub struct EventContextSync {
    _drop: Arc<MpvDropHandle>,
    /// The handle to the mpv core
    ctx: NonNull<libmpv_sys::mpv_handle>,
}

unsafe impl Send for EventContextSync {}
unsafe impl Sync for EventContextSync {}

pub struct EventContextAsync {
    inner: EventContextSync,
    waker: WakerContext,
}

pub struct EmptyEventContext;

pub trait EventContextType: sealed::EventContextType {}
impl EventContextType for EmptyEventContext {}
impl EventContextType for EventContextSync {}
impl EventContextType for EventContextAsync {}

pub trait EventContext: sealed::EventContext {}

impl EventContext for EventContextSync {}

impl EventContext for EventContextAsync {}

unsafe fn setup_waker_ptr(ctx: NonNull<libmpv_sys::mpv_handle>) -> Pin<Box<WakerLocation>> {
    let waker = Box::pin(WakerLocation {
        inner: Mutex::new(None),
        _pin: PhantomPinned,
    });
    unsafe {
        libmpv_sys::mpv_set_wakeup_callback(
            ctx.as_ptr(),
            Some(wake_callback),
            (&raw const *waker).cast_mut().cast(),
        );
    };
    waker
}

impl EventContextSync {
    pub fn enable_async(self) -> EventContextAsync {
        let location = unsafe { setup_waker_ptr(self.ctx) };
        EventContextAsync {
            inner: self,
            waker: WakerContext {
                location,
                interval: <interval::DefaultInterval as interval::Interval>::new(),
            },
        }
    }
}

impl<Protocol: ProtocolContextType> Mpv<EventContextSync, Protocol> {
    pub fn enable_async(self) -> Mpv<EventContextAsync, Protocol> {
        let location = unsafe { setup_waker_ptr(self.ctx) };
        Mpv {
            drop_handle: self.drop_handle,
            ctx: self.ctx,
            event_inline: WakerContext {
                location,
                interval: <interval::DefaultInterval as interval::Interval>::new(),
            },
            protocols_inline: self.protocols_inline,
        }
    }
}

impl<Event: sealed::EventContext, Protocol: ProtocolContextType> Mpv<Event, Protocol> {
    pub fn split_event(self) -> (Mpv<EmptyEventContext, Protocol>, Event) {
        let new = Mpv {
            drop_handle: self.drop_handle,
            ctx: self.ctx,
            event_inline: (),
            protocols_inline: self.protocols_inline,
        };
        let event = Event::exract(self.event_inline, &new);
        (new, event)
    }
}

impl<Protocol: ProtocolContextType> Mpv<EmptyEventContext, Protocol> {
    pub fn combine_event<Event: sealed::EventContext + sealed::EventContextExt>(
        self,
        event: Event,
    ) -> Result<Mpv<Event, Protocol>> {
        if event.get_ctx() != self.ctx {
            Err(Error::HandleMismatch)
        } else {
            Ok(Mpv {
                drop_handle: self.drop_handle,
                ctx: self.ctx,
                event_inline: Event::to_inlined(event),
                protocols_inline: self.protocols_inline,
            })
        }
    }
}

pub trait EventContextExt: sealed::EventContextExt {
    /// Enable an event.
    fn enable_event(&self, ev: events::EventId) -> Result<()> {
        mpv_err((), unsafe {
            libmpv_sys::mpv_request_event(self.get_ctx().as_ptr(), ev, 1)
        })
    }

    /// Enable all, except deprecated, events.
    fn enable_all_events(&self) -> Result<()> {
        for i in (2..9).chain(16..19).chain(20..23).chain(24..26) {
            self.enable_event(i)?;
        }
        Ok(())
    }

    /// Disable an event.
    fn disable_event(&self, ev: events::EventId) -> Result<()> {
        mpv_err((), unsafe {
            libmpv_sys::mpv_request_event(self.get_ctx().as_ptr(), ev, 0)
        })
    }

    /// Diable all deprecated events.
    fn disable_deprecated_events(&self) -> Result<()> {
        self.disable_event(libmpv_sys::mpv_event_id_MPV_EVENT_IDLE)?;
        Ok(())
    }

    /// Diable all events.
    fn disable_all_events(&self) -> Result<()> {
        for i in 2..26 {
            self.disable_event(i as _)?;
        }
        Ok(())
    }

    /// Observe `name` property for changes. `id` can be used to unobserve this (or many) properties
    /// again.
    fn observe_property(&self, name: &str, format: Format, id: u64) -> Result<()> {
        let name = CString::new(name)?;
        mpv_err((), unsafe {
            libmpv_sys::mpv_observe_property(
                self.get_ctx().as_ptr(),
                id,
                name.as_ptr(),
                format.as_mpv_format() as _,
            )
        })
    }

    /// Unobserve any property associated with `id`.
    fn unobserve_property(&self, id: u64) -> Result<()> {
        mpv_err((), unsafe {
            libmpv_sys::mpv_unobserve_property(self.get_ctx().as_ptr(), id)
        })
    }

    /// Wait for `timeout` seconds for an `Event`. Passing `0` as `timeout` will poll.
    /// For more information, as always, see the mpv-sys docs of `mpv_wait_event`.
    ///
    /// This function is intended to be called repeatedly in a wait-event loop.
    ///
    /// Returns `Some(Err(...))` if there was invalid utf-8, or if either an
    /// `MPV_EVENT_GET_PROPERTY_REPLY`, `MPV_EVENT_SET_PROPERTY_REPLY`, `MPV_EVENT_COMMAND_REPLY`,
    /// or `MPV_EVENT_PROPERTY_CHANGE` event failed, or if `MPV_EVENT_END_FILE` reported an error.
    fn wait_event(&mut self, timeout: f64) -> Option<Result<Event>> {
        let event = unsafe { *libmpv_sys::mpv_wait_event(self.get_ctx().as_ptr(), timeout) };
        if event.event_id != mpv_event_id::None {
            if let Err(e) = mpv_err((), event.error) {
                return Some(Err(e));
            }
        }

        match event.event_id {
            mpv_event_id::None => None,
            mpv_event_id::Shutdown => Some(Ok(Event::Shutdown)),
            mpv_event_id::LogMessage => {
                let log_message =
                    unsafe { *(event.data as *mut libmpv_sys::mpv_event_log_message) };

                let prefix = unsafe { mpv_cstr_to_str!(log_message.prefix) };
                Some(prefix.and_then(|prefix| {
                    Ok(Event::LogMessage {
                        prefix,
                        level: unsafe { mpv_cstr_to_str!(log_message.level)? },
                        text: unsafe { mpv_cstr_to_str!(log_message.text)? },
                        log_level: log_message.log_level,
                    })
                }))
            }
            mpv_event_id::GetPropertyReply => {
                let property = unsafe { *(event.data as *mut libmpv_sys::mpv_event_property) };

                let name = unsafe { mpv_cstr_to_str!(property.name) };
                Some(name.and_then(|name| {
                    // SAFETY: safe because we are passing format + data from an mpv_event_property
                    let result = unsafe { PropertyData::from_raw(property.format, property.data) }?;

                    Ok(Event::GetPropertyReply {
                        name,
                        result,
                        reply_userdata: event.reply_userdata,
                    })
                }))
            }
            mpv_event_id::SetPropertyReply => Some(mpv_err(
                Event::SetPropertyReply(event.reply_userdata),
                event.error,
            )),
            mpv_event_id::CommandReply => {
                if event.error < 0 {
                    Some(Err(event.error.into()))
                } else {
                    let data = unsafe { &*(event.data.cast::<libmpv_sys::mpv_event_command>()) };
                    Some(Ok(Event::CommandReply {
                        reply_userdata: event.reply_userdata,
                        data: MpvNode::new(data.result),
                    }))
                }
            }
            mpv_event_id::StartFile => {
                let start_file = unsafe { *(event.data as *mut libmpv_sys::mpv_event_start_file) };
                Some(Ok(Event::StartFile {
                    playlist_entry_id: start_file.playlist_entry_id,
                }))
            }
            mpv_event_id::EndFile => {
                let end_file = unsafe { *(event.data as *mut libmpv_sys::mpv_event_end_file) };

                if let Err(e) = mpv_err((), end_file.error) {
                    Some(Err(e))
                } else {
                    Some(Ok(Event::EndFile(end_file.reason as _)))
                }
            }
            mpv_event_id::FileLoaded => Some(Ok(Event::FileLoaded)),
            mpv_event_id::ClientMessage => {
                let client_message =
                    unsafe { *(event.data as *mut libmpv_sys::mpv_event_client_message) };
                let messages = unsafe {
                    slice::from_raw_parts_mut(client_message.args, client_message.num_args as _)
                };
                Some(Ok(Event::ClientMessage(
                    messages
                        .iter()
                        .map(|msg| unsafe { mpv_cstr_to_str!(*msg) })
                        .collect::<Result<Vec<_>>>()
                        .unwrap(),
                )))
            }
            mpv_event_id::VideoReconfig => Some(Ok(Event::VideoReconfig)),
            mpv_event_id::AudioReconfig => Some(Ok(Event::AudioReconfig)),
            mpv_event_id::Seek => Some(Ok(Event::Seek)),
            mpv_event_id::PlaybackRestart => Some(Ok(Event::PlaybackRestart)),
            mpv_event_id::PropertyChange => {
                let property = unsafe { *(event.data as *mut libmpv_sys::mpv_event_property) };

                // This happens if the property is not available. For example,
                // if you reached EndFile while observing a property.
                if property.format == mpv_format::None {
                    None
                } else {
                    let name = unsafe { mpv_cstr_to_str!(property.name) };
                    Some(name.and_then(|name| {
                        // SAFETY: safe because we are passing format + data from an mpv_event_property
                        let change =
                            unsafe { PropertyData::from_raw(property.format, property.data) }?;

                        Ok(Event::PropertyChange {
                            name,
                            change,
                            reply_userdata: event.reply_userdata,
                        })
                    }))
                }
            }
            mpv_event_id::QueueOverflow => Some(Ok(Event::QueueOverflow)),
            mpv_event_id::Idle => Some(Ok(Event::Idle)),
            _ => Some(Ok(Event::Deprecated(event))),
        }
    }
}

impl<T: sealed::EventContextExt> EventContextExt for T {}

fn poll(wake: &mut WakerContext, cx: &mut std::task::Context<'_>) {
    *wake.location.inner.lock().unwrap() = Some(cx.waker().clone());
    interval::Interval::poll(&mut wake.interval, cx);
}

pub trait EventContextAsyncExt:
    sealed::EventContextAsyncExt + EventContextExt + Send + Sync
{
    fn wait_event_async(&mut self) -> impl Future<Output = Result<Event>> + Send + Sync;
    fn poll_wait_event(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<Event>> {
        poll(self.get_waker(), cx);
        if let Some(v) = self.wait_event(0.0) {
            Poll::Ready(v)
        } else {
            Poll::Pending
        }
    }
}

impl<T: sealed::EventContextAsyncExt + EventContextExt + Send + Sync> EventContextAsyncExt for T {
    async fn wait_event_async(&mut self) -> Result<Event> {
        poll_fn(|cx| {
            poll(self.get_waker(), cx);
            Poll::Ready(())
        })
        .await;
        if let Some(v) = self.wait_event(0.0) {
            return v;
        }
        pending().await
    }
}

mod sealed {
    use std::ptr::NonNull;

    use super::{
        EmptyEventContext, EventContextAsync, EventContextSync, Mpv, WakerContext,
        protocol::ProtocolContextType,
    };

    pub trait EventContextType {
        type Inlined;
    }
    impl EventContextType for EventContextSync {
        type Inlined = ();
    }
    impl EventContextType for EventContextAsync {
        type Inlined = WakerContext;
    }
    impl EventContextType for EmptyEventContext {
        type Inlined = ();
    }

    pub trait EventContext: super::EventContextType {
        fn exract<Protocol: ProtocolContextType>(
            inline: Self::Inlined,
            cx: &Mpv<EmptyEventContext, Protocol>,
        ) -> Self;
        fn to_inlined(self) -> Self::Inlined;
    }

    impl EventContext for EventContextSync {
        fn exract<Protocol: ProtocolContextType>(
            _inline: Self::Inlined,
            cx: &Mpv<EmptyEventContext, Protocol>,
        ) -> Self {
            EventContextSync {
                ctx: cx.ctx,
                _drop: cx.drop_handle.clone(),
            }
        }
        fn to_inlined(self) -> Self::Inlined {}
    }

    impl EventContext for EventContextAsync {
        fn exract<Protocol: ProtocolContextType>(
            inline: Self::Inlined,
            cx: &Mpv<EmptyEventContext, Protocol>,
        ) -> Self {
            EventContextAsync {
                inner: EventContextSync::exract((), cx),
                waker: inline,
            }
        }

        fn to_inlined(self) -> Self::Inlined {
            self.waker
        }
    }
    /// # Safety
    /// ctx must be valid
    pub unsafe trait EventContextExt {
        ///this must return a valid handle
        fn get_ctx(&self) -> NonNull<libmpv_sys::mpv_handle>;
    }
    unsafe impl EventContextExt for EventContextSync {
        fn get_ctx(&self) -> NonNull<libmpv_sys::mpv_handle> {
            self.ctx
        }
    }

    unsafe impl EventContextExt for EventContextAsync {
        fn get_ctx(&self) -> NonNull<libmpv_sys::mpv_handle> {
            self.inner.ctx
        }
    }

    unsafe impl<Protocol: ProtocolContextType> EventContextExt for Mpv<EventContextSync, Protocol> {
        fn get_ctx(&self) -> NonNull<libmpv_sys::mpv_handle> {
            self.ctx
        }
    }

    unsafe impl<Protocol: ProtocolContextType> EventContextExt for Mpv<EventContextAsync, Protocol> {
        fn get_ctx(&self) -> NonNull<libmpv_sys::mpv_handle> {
            self.ctx
        }
    }
    pub trait EventContextAsyncExt: EventContextExt {
        fn get_waker(&mut self) -> &mut WakerContext;
    }
    impl EventContextAsyncExt for EventContextAsync {
        fn get_waker(&mut self) -> &mut WakerContext {
            &mut self.waker
        }
    }
    impl<Protocol: ProtocolContextType> EventContextAsyncExt for Mpv<EventContextAsync, Protocol> {
        fn get_waker(&mut self) -> &mut WakerContext {
            &mut self.event_inline
        }
    }
}

mod interval {
    use std::{task::Context, time::Duration};

    pub trait Interval {
        fn new() -> Self;
        fn poll(&mut self, cx: &mut Context);
    }

    #[cfg(feature = "tokio")]
    impl Interval for tokio::time::Interval {
        fn new() -> Self {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            interval
        }

        fn poll(&mut self, cx: &mut Context) {
            while self.poll_tick(cx).is_ready() {}
        }
    }

    #[cfg(feature = "tokio")]
    pub type DefaultInterval = tokio::time::Interval;
}
