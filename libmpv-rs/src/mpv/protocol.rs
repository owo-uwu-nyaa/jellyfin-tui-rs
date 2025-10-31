// Copyright (C) 2016  ParadoxSpiral
//
// This file is part of mpv-rs.
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

use super::*;

use std::alloc::{self, Layout};
use std::ffi::CString;
use std::mem;
use std::os::raw as ctype;
use std::panic;
use std::panic::RefUnwindSafe;
use std::ptr::{self, NonNull};
use std::slice;
use std::sync::Mutex;

/// Return a persistent `T` that is passed to all other `Stream*` functions, panic on errors.
pub type StreamOpen<T, U> = fn(&mut U, &str) -> T;
/// Do any necessary cleanup.
pub type StreamClose<T> = fn(Box<T>);
/// Seek to the given offset. Return the new offset, or either `MpvError::Generic` if seeking
/// failed or panic.
pub type StreamSeek<T> = fn(&mut T, i64) -> i64;
/// Target buffer with fixed capacity.
/// Return either the number of read bytes, `0` on EOF, or either `-1` or panic on error.
pub type StreamRead<T> = fn(&mut T, &mut [ctype::c_char]) -> i64;
/// Return the total size of the stream in bytes. Panic on error.
pub type StreamSize<T> = fn(&mut T) -> i64;

unsafe extern "C" fn open_wrapper<T, U>(
    user_data: *mut ctype::c_void,
    uri: *mut ctype::c_char,
    info: *mut libmpv_sys::mpv_stream_cb_info,
) -> ctype::c_int
where
    T: RefUnwindSafe,
    U: RefUnwindSafe,
{
    let data = user_data as *mut ProtocolData<T, U>;
    let ret = unsafe {
        (*info).cookie = user_data;
        (*info).read_fn = Some(read_wrapper::<T, U>);
        (*info).seek_fn = Some(seek_wrapper::<T, U>);
        (*info).size_fn = Some(size_wrapper::<T, U>);
        (*info).close_fn = Some(close_wrapper::<T, U>);

        panic::catch_unwind(|| {
            let uri = mpv_cstr_to_str(uri as *const _).unwrap();
            ptr::write(
                (*data).cookie,
                ((*data).open_fn)(&mut (*data).user_data, uri),
            );
        })
    };

    if ret.is_ok() {
        0
    } else {
        mpv_error::Generic as _
    }
}

unsafe extern "C" fn read_wrapper<T, U>(
    cookie: *mut ctype::c_void,
    buf: *mut ctype::c_char,
    nbytes: u64,
) -> i64
where
    T: RefUnwindSafe,
    U: RefUnwindSafe,
{
    let data = cookie as *mut ProtocolData<T, U>;

    let ret = panic::catch_unwind(|| unsafe {
        let slice = slice::from_raw_parts_mut(buf, nbytes as _);
        ((*data).read_fn)(&mut *(*data).cookie, slice)
    });
    ret.unwrap_or(-1)
}

unsafe extern "C" fn seek_wrapper<T, U>(cookie: *mut ctype::c_void, offset: i64) -> i64
where
    T: RefUnwindSafe,
    U: RefUnwindSafe,
{
    let data = cookie as *mut ProtocolData<T, U>;

    if unsafe { (*data).seek_fn }.is_none() {
        return mpv_error::Unsupported as _;
    }

    let ret = panic::catch_unwind(|| unsafe {
        (*(*data).seek_fn.as_ref().unwrap())(&mut *(*data).cookie, offset)
    });
    if let Ok(ret) = ret {
        ret
    } else {
        mpv_error::Generic as _
    }
}

unsafe extern "C" fn size_wrapper<T, U>(cookie: *mut ctype::c_void) -> i64
where
    T: RefUnwindSafe,
    U: RefUnwindSafe,
{
    let data = cookie as *mut ProtocolData<T, U>;

    if unsafe { (*data).size_fn }.is_none() {
        return mpv_error::Unsupported as _;
    }

    let ret = panic::catch_unwind(|| unsafe {
        (*(*data).size_fn.as_ref().unwrap())(&mut *(*data).cookie)
    });
    if let Ok(ret) = ret {
        ret
    } else {
        mpv_error::Unsupported as _
    }
}

#[allow(unused_must_use)]
unsafe extern "C" fn close_wrapper<T, U>(cookie: *mut ctype::c_void)
where
    T: RefUnwindSafe,
    U: RefUnwindSafe,
{
    let data = unsafe { Box::from_raw(cookie as *mut ProtocolData<T, U>) };

    panic::catch_unwind(|| (data.close_fn)(unsafe { Box::from_raw(data.cookie) }));
}

struct ProtocolData<T, U> {
    cookie: *mut T,
    user_data: U,

    open_fn: StreamOpen<T, U>,
    close_fn: StreamClose<T>,
    read_fn: StreamRead<T>,
    seek_fn: Option<StreamSeek<T>>,
    size_fn: Option<StreamSize<T>>,
}

pub trait ProtocolContextType: sealed::ProtocolContextType {
    type Inlined;
}

/// This context holds state relevant to custom protocols.
/// It is created by calling `Mpv::create_protocol_context`.
pub struct ProtocolContext<T: RefUnwindSafe, U: RefUnwindSafe> {
    _drop: Arc<MpvDropHandle>,
    ctx: NonNull<libmpv_sys::mpv_handle>,
    protocols: Mutex<Vec<Protocol<T, U>>>,
}

impl<T: RefUnwindSafe, U: RefUnwindSafe> ProtocolContextType for ProtocolContext<T, U> {
    type Inlined = Mutex<Vec<Protocol<T, U>>>;
}

unsafe impl<T: RefUnwindSafe, U: RefUnwindSafe> Send for ProtocolContext<T, U> {}
unsafe impl<T: RefUnwindSafe, U: RefUnwindSafe> Sync for ProtocolContext<T, U> {}

pub struct EmptyProtocolContext;
impl ProtocolContextType for EmptyProtocolContext {
    type Inlined = ();
}

pub struct UninitProtocolContext {
    _drop: Arc<MpvDropHandle>,
    ctx: NonNull<libmpv_sys::mpv_handle>,
}

impl ProtocolContextType for UninitProtocolContext {
    type Inlined = ();
}

impl UninitProtocolContext {
    pub fn enable_protocol<T: RefUnwindSafe, U: RefUnwindSafe>(self) -> ProtocolContext<T, U> {
        ProtocolContext {
            _drop: self._drop,
            ctx: self.ctx,
            protocols: Mutex::new(Vec::new()),
        }
    }
}

impl<Event: EventContextType> Mpv<Event, UninitProtocolContext> {
    pub fn enable_protocol<T: RefUnwindSafe, U: RefUnwindSafe>(
        self,
    ) -> Mpv<Event, ProtocolContext<T, U>> {
        Mpv {
            drop_handle: self.drop_handle,
            ctx: self.ctx,
            event_inline: self.event_inline,
            protocols_inline: Mutex::new(Vec::new()),
        }
    }
}

impl<Event: EventContextType, Protocol: sealed::ProtocolCx> Mpv<Event, Protocol> {
    pub fn split_protocol(self) -> (Mpv<Event, EmptyProtocolContext>, Protocol) {
        let new = Mpv {
            drop_handle: self.drop_handle,
            ctx: self.ctx,
            event_inline: self.event_inline,
            protocols_inline: (),
        };
        let protocol = Protocol::extract(self.protocols_inline, &new);
        (new, protocol)
    }
}

impl<Event: EventContextType> Mpv<Event, EmptyProtocolContext> {
    pub fn combine_protocol<Protocol: sealed::ProtocolCx + sealed::ProtocolContextCtx>(
        self,
        protocol: Protocol,
    ) -> Result<Mpv<Event, Protocol>> {
        if self.ctx != protocol.get_ctx() {
            Err(Error::HandleMismatch)
        } else {
            Ok(Mpv {
                drop_handle: self.drop_handle,
                ctx: self.ctx,
                event_inline: self.event_inline,
                protocols_inline: Protocol::to_inline(protocol),
            })
        }
    }
}

pub trait ProtocolContextExt<T: RefUnwindSafe, U: RefUnwindSafe>:
    sealed::ProtocolContextExt<T, U>
{
    /// Register a custom `Protocol`. Once a protocol has been registered, it lives as long as
    /// `Mpv`.
    ///
    /// Returns `Error::Mpv(MpvError::InvalidParameter)` if a protocol with the same name has
    /// already been registered.
    fn register(&self, protocol: Protocol<T, U>) -> Result<()> {
        let mut protocols = self.get_protocols().lock().unwrap();
        protocol.register(self.get_ctx().as_ptr())?;
        protocols.push(protocol);
        Ok(())
    }
}

impl<T: RefUnwindSafe, U: RefUnwindSafe, P: sealed::ProtocolContextExt<T, U>>
    ProtocolContextExt<T, U> for P
{
}

/// `Protocol` holds all state used by a custom protocol.
pub struct Protocol<T: Sized + RefUnwindSafe, U: RefUnwindSafe> {
    name: String,
    data: *mut ProtocolData<T, U>,
}

impl<T: RefUnwindSafe, U: RefUnwindSafe> Protocol<T, U> {
    /// `name` is the prefix of the protocol, e.g. `name://path`.
    ///
    /// `user_data` is data that will be passed to `open_fn`.
    ///
    /// # Safety
    /// Do not call libmpv functions in any supplied function.
    /// All panics of the provided functions are catched and can be used as generic error returns.
    pub unsafe fn new(
        name: String,
        user_data: U,
        open_fn: StreamOpen<T, U>,
        close_fn: StreamClose<T>,
        read_fn: StreamRead<T>,
        seek_fn: Option<StreamSeek<T>>,
        size_fn: Option<StreamSize<T>>,
    ) -> Protocol<T, U> {
        let c_layout = Layout::from_size_align(mem::size_of::<T>(), mem::align_of::<T>()).unwrap();
        let cookie = unsafe { alloc::alloc(c_layout) as *mut T };
        let data = Box::into_raw(Box::new(ProtocolData {
            cookie,
            user_data,

            open_fn,
            close_fn,
            read_fn,
            seek_fn,
            size_fn,
        }));

        Protocol { name, data }
    }

    fn register(&self, ctx: *mut libmpv_sys::mpv_handle) -> Result<()> {
        let name = CString::new(&self.name[..])?;
        unsafe {
            mpv_err(
                (),
                libmpv_sys::mpv_stream_cb_add_ro(
                    ctx,
                    name.as_ptr(),
                    self.data as *mut _,
                    Some(open_wrapper::<T, U>),
                ),
            )
        }
    }
}

mod sealed {
    use std::{panic::RefUnwindSafe, ptr::NonNull, sync::Mutex};

    use super::{
        EmptyProtocolContext, EventContextType, Mpv, Protocol, ProtocolContext,
        UninitProtocolContext,
    };

    pub trait ProtocolContextType {}
    impl ProtocolContextType for EmptyProtocolContext {}
    impl ProtocolContextType for UninitProtocolContext {}
    impl<T: RefUnwindSafe, U: RefUnwindSafe> ProtocolContextType for ProtocolContext<T, U> {}

    pub trait ProtocolCx: super::ProtocolContextType {
        fn extract<Event: EventContextType>(
            inline: Self::Inlined,
            cx: &Mpv<Event, EmptyProtocolContext>,
        ) -> Self;
        fn to_inline(self) -> Self::Inlined;
    }
    impl ProtocolCx for UninitProtocolContext {
        fn extract<Event: EventContextType>(
            _inline: Self::Inlined,
            cx: &Mpv<Event, EmptyProtocolContext>,
        ) -> Self {
            UninitProtocolContext {
                _drop: cx.drop_handle.clone(),
                ctx: cx.ctx,
            }
        }
        fn to_inline(self) -> Self::Inlined {}
    }

    impl<T: RefUnwindSafe, U: RefUnwindSafe> ProtocolCx for ProtocolContext<T, U> {
        fn extract<Event: EventContextType>(
            inline: Self::Inlined,
            cx: &Mpv<Event, EmptyProtocolContext>,
        ) -> Self {
            ProtocolContext {
                _drop: cx.drop_handle.clone(),
                ctx: cx.ctx,
                protocols: inline,
            }
        }

        fn to_inline(self) -> Self::Inlined {
            self.protocols
        }
    }
    /// # Safety
    /// ctx must be valid
    pub unsafe trait ProtocolContextCtx {
        ///this must return a valid handle
        fn get_ctx(&self) -> NonNull<libmpv_sys::mpv_handle>;
    }
    unsafe impl<T: RefUnwindSafe, U: RefUnwindSafe> ProtocolContextCtx for ProtocolContext<T, U> {
        fn get_ctx(&self) -> NonNull<libmpv_sys::mpv_handle> {
            self.ctx
        }
    }
    unsafe impl<T: RefUnwindSafe, U: RefUnwindSafe, Event: EventContextType> ProtocolContextCtx
        for Mpv<Event, ProtocolContext<T, U>>
    {
        fn get_ctx(&self) -> NonNull<libmpv_sys::mpv_handle> {
            self.ctx
        }
    }
    unsafe impl ProtocolContextCtx for UninitProtocolContext {
        fn get_ctx(&self) -> NonNull<libmpv_sys::mpv_handle> {
            self.ctx
        }
    }
    pub trait ProtocolContextExt<T: RefUnwindSafe, U: RefUnwindSafe>: ProtocolContextCtx {
        fn get_protocols(&self) -> &Mutex<Vec<Protocol<T, U>>>;
    }

    impl<T: RefUnwindSafe, U: RefUnwindSafe> ProtocolContextExt<T, U> for ProtocolContext<T, U> {
        fn get_protocols(&self) -> &Mutex<Vec<Protocol<T, U>>> {
            &self.protocols
        }
    }
    impl<T: RefUnwindSafe, U: RefUnwindSafe, Event: EventContextType> ProtocolContextExt<T, U>
        for Mpv<Event, ProtocolContext<T, U>>
    {
        fn get_protocols(&self) -> &Mutex<Vec<Protocol<T, U>>> {
            &self.protocols_inline
        }
    }
}
