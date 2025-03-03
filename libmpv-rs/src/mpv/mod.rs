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

use std::ffi::CStr;
use std::ptr::null_mut;
use std::sync::Arc;

macro_rules! mpv_cstr_to_str {
    ($cstr: expr) => {
        std::ffi::CStr::from_ptr($cstr)
            .to_str()
            .map_err(Error::from)
    };
}

mod errors;

/// Event handling
pub mod events;
pub mod node;
/// Custom protocols (`protocol://$url`) for playback
pub mod protocol;
/// Custom rendering
#[cfg(feature = "render")]
pub mod render;

use events::{EventContextSync, EventContextType};
use node::{BorrowingMpvNode, BorrowingMpvNodeList, ToNode};
use protocol::{ProtocolContextType, UninitProtocolContext};

pub use self::errors::*;
use super::*;

use std::{
    ffi::CString,
    mem::MaybeUninit,
    ops::Deref,
    os::raw as ctype,
    ptr::{self, NonNull},
    result::Result as StdResult,
};

fn mpv_err<T>(ret: T, err: ctype::c_int) -> Result<T> {
    if err == 0 {
        Ok(ret)
    } else {
        Err(Error::Raw(err))
    }
}

/**
 * This trait describes which types are allowed to be passed to getter mpv APIs.
 *
 * # Safety
 * The result of get_format must match the pointer consumed through get_from_c_void.
 *  */
pub unsafe trait GetData: Sized {
    fn get_from_c_void<T, F: FnMut(*mut ctype::c_void) -> Result<T>>(mut fun: F) -> Result<Self> {
        let mut val = MaybeUninit::uninit();
        let _ = fun(val.as_mut_ptr() as *mut _)?;
        Ok(unsafe { val.assume_init() })
    }
    fn get_format() -> Format;
}

unsafe impl GetData for f64 {
    fn get_format() -> Format {
        Format::Double
    }
}

unsafe impl GetData for i64 {
    fn get_format() -> Format {
        Format::Int64
    }
}

unsafe impl GetData for bool {
    fn get_format() -> Format {
        Format::Flag
    }
}

unsafe impl GetData for String {
    fn get_from_c_void<T, F: FnMut(*mut ctype::c_void) -> Result<T>>(mut fun: F) -> Result<String> {
        let ptr = &mut ptr::null();
        let _ = fun(ptr as *mut *const ctype::c_char as _)?;

        let ret = unsafe { mpv_cstr_to_str!(*ptr) }?.to_owned();
        unsafe { libmpv_sys::mpv_free(*ptr as *mut _) };
        Ok(ret)
    }

    fn get_format() -> Format {
        Format::String
    }
}

/// Wrapper around an `&str` returned by mpv, that properly deallocates it with mpv's allocator.
#[derive(Debug, Hash, Eq, PartialEq)]
pub struct MpvStr<'a>(&'a str);
impl Deref for MpvStr<'_> {
    type Target = str;

    fn deref(&self) -> &str {
        self.0
    }
}
impl Drop for MpvStr<'_> {
    fn drop(&mut self) {
        unsafe { libmpv_sys::mpv_free(self.0.as_ptr() as *mut u8 as _) };
    }
}

unsafe impl<'a> GetData for MpvStr<'a> {
    fn get_from_c_void<T, F: FnMut(*mut ctype::c_void) -> Result<T>>(
        mut fun: F,
    ) -> Result<MpvStr<'a>> {
        let ptr = &mut ptr::null();
        let _ = fun(ptr as *mut *const ctype::c_char as _)?;

        Ok(MpvStr(unsafe { mpv_cstr_to_str!(*ptr) }?))
    }

    fn get_format() -> Format {
        Format::String
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
/// Subset of `mpv_format` used by the public API.
pub enum Format {
    String,
    Flag,
    Int64,
    Double,
    Node,
    Map,
}

impl Format {
    fn as_mpv_format(&self) -> MpvFormat {
        match *self {
            Format::String => mpv_format::String,
            Format::Flag => mpv_format::Flag,
            Format::Int64 => mpv_format::Int64,
            Format::Double => mpv_format::Double,
            Format::Node => mpv_format::Node,
            Format::Map => mpv_format::Map,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// How a `File` is inserted into the playlist.
pub enum FileState {
    /// Replace the current track.
    Replace,
    /// Append to the current playlist.
    Append,
    /// If current playlist is empty: play, otherwise append to playlist.
    AppendPlay,
}

pub enum Cycle {
    Up,
    Down,
}
impl Cycle {
    fn to_cstr(&self) -> &'static CStr {
        match self {
            Cycle::Up => c"up",
            Cycle::Down => c"down",
        }
    }
}

/// Context passed to the `initializer` of `Mpv::with_initialzer`.
pub struct MpvInitializer {
    ctx: NonNull<libmpv_sys::mpv_handle>,
}

impl MpvInitializer {
    /// Set the value of a property.
    pub fn set_option<'n>(&self, name: &CStr, data: impl ToNode<'n>) -> Result<()> {
        mpv_err((), unsafe {
            libmpv_sys::mpv_set_option(
                self.ctx.as_ptr(),
                name.as_ptr(),
                libmpv_sys::mpv_format_MPV_FORMAT_NODE,
                data.to_node().node().cast(),
            )
        })
    }
}

struct MpvDropHandle {
    ctx: NonNull<libmpv_sys::mpv_handle>,
}

impl Drop for MpvDropHandle {
    fn drop(&mut self) {
        unsafe {
            libmpv_sys::mpv_terminate_destroy(self.ctx.as_ptr());
        }
    }
}

unsafe impl Send for MpvDropHandle {}
unsafe impl Sync for MpvDropHandle {}

/// The central mpv context.
pub struct Mpv<
    Event: EventContextType = EventContextSync,
    Protocol: ProtocolContextType = UninitProtocolContext,
> {
    drop_handle: Arc<MpvDropHandle>,
    ctx: NonNull<libmpv_sys::mpv_handle>,
    event_inline: Event::Inlined,
    protocols_inline: Protocol::Inlined,
}

unsafe impl<Event: EventContextType, Protocol: ProtocolContextType> Send for Mpv<Event, Protocol> {}
unsafe impl<Event: EventContextType, Protocol: ProtocolContextType> Sync for Mpv<Event, Protocol> {}

impl Mpv {
    /// Create a new `Mpv`.
    /// The default settings can be probed by running: `$ mpv --show-profile=libmpv`.
    pub fn new() -> Result<Mpv<EventContextSync, UninitProtocolContext>> {
        Self::with_initializer(|_| Ok(()))
    }

    /// Create a new `Mpv`.
    /// The same as `Mpv::new`, but you can set properties before `Mpv` is initialized.
    pub fn with_initializer<E: From<Error>, F: FnOnce(MpvInitializer) -> StdResult<(), E>>(
        initializer: F,
    ) -> StdResult<Mpv<EventContextSync, UninitProtocolContext>, E> {
        let api_version = unsafe { libmpv_sys::mpv_client_api_version() };
        if crate::MPV_CLIENT_API_MAJOR != api_version >> 16 {
            return Err(Error::VersionMismatch {
                linked: crate::MPV_CLIENT_API_VERSION,
                loaded: api_version,
            }
            .into());
        }

        let ctx = unsafe { libmpv_sys::mpv_create() };
        if ctx.is_null() {
            return Err(Error::Null.into());
        }

        let ctx = unsafe { NonNull::new_unchecked(ctx) };
        let drop_handle = Arc::new(MpvDropHandle { ctx });

        initializer(MpvInitializer { ctx })?;
        mpv_err((), unsafe { libmpv_sys::mpv_initialize(ctx.as_ptr()) })?;
        Ok(Mpv {
            ctx,
            drop_handle,
            event_inline: (),
            protocols_inline: (),
        })
    }
}

impl<Event: EventContextType, Protocol: ProtocolContextType> Mpv<Event, Protocol> {
    /// Load a configuration file. The path has to be absolute, and a file.
    pub fn load_config(&self, path: &str) -> Result<()> {
        let file = CString::new(path)?.into_raw();
        mpv_err((), unsafe {
            libmpv_sys::mpv_load_config_file(self.ctx.as_ptr(), file)
        })
    }

    /// Send a command to the `Mpv` instance.
    pub fn command(&self, args: &[BorrowingMpvNode<'_>]) -> Result<()> {
        mpv_err((), unsafe {
            libmpv_sys::mpv_command_node(
                self.ctx.as_ptr(),
                BorrowingMpvNodeList::new(args).to_node().node(),
                null_mut(),
            )
        })
    }

    /// Send a command to the `Mpv` instance.
    pub fn command_async(&self, args: &[BorrowingMpvNode<'_>], reply_userdata: u64) -> Result<()> {
        mpv_err((), unsafe {
            libmpv_sys::mpv_command_node_async(
                self.ctx.as_ptr(),
                reply_userdata,
                BorrowingMpvNodeList::new(args).to_node().node(),
            )
        })
    }

    /// Set the value of a property.
    pub fn set_property<'n>(&self, name: &CStr, data: impl ToNode<'n>) -> Result<()> {
        mpv_err((), unsafe {
            libmpv_sys::mpv_set_property(
                self.ctx.as_ptr(),
                name.as_ptr(),
                libmpv_sys::mpv_format_MPV_FORMAT_NODE,
                data.to_node().node().cast(),
            )
        })
    }

    /// Get the value of a property.
    pub fn get_property<T: GetData>(&self, name: &str) -> Result<T> {
        let name = CString::new(name)?;

        let format = T::get_format().as_mpv_format() as _;
        T::get_from_c_void(|ptr| {
            mpv_err((), unsafe {
                libmpv_sys::mpv_get_property(self.ctx.as_ptr(), name.as_ptr(), format, ptr)
            })
        })
    }

    /// Get the value of a property.
    pub fn get_property_async<T: GetData>(&self, name: &str, reply_userdata: u64) -> Result<()> {
        let name = CString::new(name)?;

        let format = T::get_format().as_mpv_format() as _;
        mpv_err((), unsafe {
            libmpv_sys::mpv_get_property_async(
                self.ctx.as_ptr(),
                reply_userdata,
                name.as_ptr(),
                format,
            )
        })
    }

    /// Internal time in microseconds, this has an arbitrary offset, and will never go backwards.
    ///
    /// This can be called at any time, even if it was stated that no API function should be called.
    pub fn get_internal_time(&self) -> i64 {
        unsafe { libmpv_sys::mpv_get_time_us(self.ctx.as_ptr()) }
    }

    // --- Convenience property functions ---
    //

    /// Add -or subtract- any value from a property. Over/underflow clamps to max/min.
    pub fn add_property(&self, property: &CStr, value: i64) -> Result<()> {
        self.command(&[c"add".to_node(), property.to_node(), value.to_node()])
    }

    /// Cycle through a given property. `up` specifies direction. On
    /// overflow, set the property back to the minimum, on underflow set it to the maximum.
    pub fn cycle_property(&self, property: &CStr, direction: Cycle) -> Result<()> {
        self.command(&[
            c"cycle".to_node(),
            property.to_node(),
            direction.to_cstr().to_node(),
        ])
    }

    /// Multiply any property with any positive factor.
    pub fn multiply_property(&self, property: &CStr, factor: i64) -> Result<()> {
        self.command(&[c"multiply".to_node(), property.to_node(), factor.to_node()])
    }

    pub fn set_pause(&self, pause: bool) -> Result<()> {
        self.set_property(c"pause", pause)
    }

    /// Pause playback at runtime.
    pub fn pause(&self) -> Result<()> {
        self.set_pause(true)
    }

    /// Unpause playback at runtime.
    pub fn unpause(&self) -> Result<()> {
        self.set_pause(false)
    }

    // --- Seek functions ---
    pub fn seek(&self, position: f64, flags: &CStr) -> Result<()> {
        self.command(&[c"seek".to_node(), position.to_node(), flags.to_node()])
    }

    /// Seek forward relatively from current position in seconds.
    /// This is less exact than `seek_absolute`, see [mpv manual]
    /// (https://mpv.io/manual/master/#command-interface-
    /// [relative|absolute|absolute-percent|relative-percent|exact|keyframes]).
    pub fn seek_forward(&self, secs: f64) -> Result<()> {
        self.seek(secs, c"relative")
    }

    /// See `seek_forward`.
    pub fn seek_backward(&self, secs: f64) -> Result<()> {
        self.seek_forward(-secs)
    }

    /// Seek to a given absolute secs.
    pub fn seek_absolute(&self, secs: f64) -> Result<()> {
        self.seek(secs, c"absolute")
    }

    /// Seek to a given relative percent position (may be negative).
    /// If `percent` of the playtime is bigger than the remaining playtime, the next file is played.
    /// out of bounds values are clamped to either 0 or 100.
    pub fn seek_percent(&self, percent: f64) -> Result<()> {
        self.seek(percent, c"relative-percent")
    }

    /// Seek to the given percentage of the playtime.
    pub fn seek_percent_absolute(&self, percent: f64) -> Result<()> {
        self.seek(percent, c"absolute-percent")
    }

    /// Revert the previous `seek_` call, can also revert itself.
    pub fn seek_revert(&self) -> Result<()> {
        self.command(&[c"seek-revert".to_node()])
    }

    /// Mark the current position as the position that will be seeked to by `seek_revert`.
    pub fn seek_revert_mark(&self) -> Result<()> {
        self.command(&[c"seek-revert".to_node(), c"mark".to_node()])
    }

    /// Mark the current position as the position that will be seeked to by `seek_revert`.
    pub fn seek_revert_mark_permanent(&self) -> Result<()> {
        self.command(&[c"seek-revert".to_node(), c"mark-permanent".to_node()])
    }

    /// Seek exactly one frame, and pause.
    /// Noop on audio only streams.
    pub fn seek_frame(&self) -> Result<()> {
        self.command(&[c"frame-step".to_node()])
    }

    /// See `seek_frame`.
    /// [Note performance considerations.](https://mpv.io/manual/master/#command-interface-frame-back-step)
    pub fn seek_frame_backward(&self) -> Result<()> {
        self.command(&[c"frame-back-step".to_node()])
    }

    // --- Screenshot functions ---
    //

    /// "Save the video image, in its original resolution, and with subtitles.
    /// Some video outputs may still include the OSD in the output under certain circumstances.".
    ///
    /// "[O]ptionally save it to a given file. The format of the file will be
    /// guessed by the extension (and --screenshot-format is ignored - the behaviour when the
    /// extension is missing or unknown is arbitrary). If the file already exists, it's overwritten.
    /// Like all input command parameters, the filename is subject to property expansion as
    /// described in [Property Expansion](https://mpv.io/manual/master/#property-expansion)."
    pub fn screenshot_subtitles(&self, path: Option<&CStr>) -> Result<()> {
        if let Some(path) = path {
            self.command(&[c"screenshot-to-file".to_node(), path.to_node()])
        } else {
            self.command(&[c"screenshot".to_node()])
        }
    }

    /// "Like subtitles, but typically without OSD or subtitles. The exact behavior
    /// depends on the selected video output."
    pub fn screenshot_video(&self, path: Option<&CStr>) -> Result<()> {
        if let Some(path) = path {
            self.command(&[
                c"screenshot-to-file".to_node(),
                path.to_node(),
                c"video".to_node(),
            ])
        } else {
            self.command(&[c"screenshot".to_node(), c"video".to_node()])
        }
    }

    /// "Save the contents of the mpv window. Typically scaled, with OSD and subtitles. The exact
    /// behaviour depends on the selected video output, and if no support is available,
    /// this will act like video.".
    pub fn screenshot_window(&self, path: Option<&CStr>) -> Result<()> {
        if let Some(path) = path {
            self.command(&[
                c"screenshot-to-file".to_node(),
                path.to_node(),
                c"window".to_node(),
            ])
        } else {
            self.command(&[c"screenshot".to_node(), c"window".to_node()])
        }
    }

    // --- Playlist functions ---
    //

    /// Play the next item of the current playlist.
    /// Does nothing if the current item is the last item.
    pub fn playlist_next_weak(&self) -> Result<()> {
        self.command(&[c"playlist-next".to_node(), c"weak".to_node()])
    }

    /// Play the next item of the current playlist.
    /// Terminates playback if the current item is the last item.
    pub fn playlist_next_force(&self) -> Result<()> {
        self.command(&[c"playlist-next".to_node(), c"force".to_node()])
    }

    /// See `playlist_next_weak`.
    pub fn playlist_previous_weak(&self) -> Result<()> {
        self.command(&[c"playlist-prev".to_node(), c"weak".to_node()])
    }

    /// See `playlist_next_force`.
    pub fn playlist_previous_force(&self) -> Result<()> {
        self.command(&[c"playlist-prev".to_node(), c"force".to_node()])
    }

    pub fn playlist_replace(&self, file: &CStr) -> Result<()> {
        self.command(&[c"loadfile".to_node(), file.to_node(), c"replace".to_node()])
    }

    pub fn playlist_append(&self, file: &CStr) -> Result<()> {
        self.command(&[c"loadfile".to_node(), file.to_node(), c"append".to_node()])
    }
    pub fn playlist_append_play(&self, file: &CStr) -> Result<()> {
        self.command(&[
            c"loadfile".to_node(),
            file.to_node(),
            c"append-play".to_node(),
        ])
    }

    pub fn playlist_insert_at(&self, file: &CStr, index: i64) -> Result<()> {
        self.command(&[
            c"loadfile".to_node(),
            file.to_node(),
            c"insert-at".to_node(),
            index.to_node(),
        ])
    }

    /// Load the given playlist file, that either replaces the current playlist, or appends to it.
    pub fn playlist_load_list(&self, path: &CStr, replace: bool) -> Result<()> {
        let action = if replace { c"replace" } else { c"append" };
        self.command(&[c"loadlist".to_node(), path.to_node(), action.to_node()])
    }

    /// Remove every, except the current, item from the playlist.
    pub fn playlist_clear(&self) -> Result<()> {
        self.command(&[c"playlist-clear".to_node()])
    }

    /// Remove the currently selected item from the playlist.
    pub fn playlist_remove_current(&self) -> Result<()> {
        self.command(&[c"playlist-remove".to_node(), c"current".to_node()])
    }

    /// Remove item at `position` from the playlist.
    pub fn playlist_remove_index(&self, position: i64) -> Result<()> {
        self.command(&[c"playlist-remove".to_node(), position.to_node()])
    }

    /// Move item `old` to the position of item `new`.
    pub fn playlist_move(&self, old: i64, new: i64) -> Result<()> {
        self.command(&[c"playlist-move".to_node(), old.to_node(), new.to_node()])
    }

    /// Shuffle the playlist.
    pub fn playlist_shuffle(&self) -> Result<()> {
        self.command(&[c"playlist-shuffle".to_node()])
    }

    // --- Subtitle functions ---
    //

    /// Add and select the subtitle immediately.
    /// Specifying a language requires specifying a title.
    ///
    /// # Panics
    /// If a language but not title was specified.
    pub fn subtitle_add_select(
        &self,
        path: &CStr,
        title: Option<&CStr>,
        lang: Option<&CStr>,
    ) -> Result<()> {
        match (title, lang) {
            (None, None) => {
                self.command(&[c"sub-add".to_node(), path.to_node(), c"select".to_node()])
            }
            (Some(t), None) => self.command(&[
                c"sub-add".to_node(),
                path.to_node(),
                c"select".to_node(),
                t.to_node(),
            ]),
            (None, Some(_)) => panic!("Given subtitle language, but missing title"),
            (Some(t), Some(l)) => self.command(&[
                c"sub-add".to_node(),
                path.to_node(),
                c"select".to_node(),
                t.to_node(),
                l.to_node(),
            ]),
        }
    }

    /// See `AddSelect`. "Don't select the subtitle.
    /// (Or in some special situations, let the default stream selection mechanism decide.)".
    ///
    /// Returns an `Error::InvalidArgument` if a language, but not a title, was provided.
    ///
    /// # Panics
    /// If a language but not title was specified.
    pub fn subtitle_add_auto(
        &self,
        path: &CStr,
        title: Option<&CStr>,
        lang: Option<&CStr>,
    ) -> Result<()> {
        match (title, lang) {
            (None, None) => {
                self.command(&[c"sub-add".to_node(), path.to_node(), c"auto".to_node()])
            }
            (Some(t), None) => self.command(&[
                c"sub-add".to_node(),
                path.to_node(),
                c"auto".to_node(),
                t.to_node(),
            ]),
            (Some(t), Some(l)) => self.command(&[
                c"sub-add".to_node(),
                path.to_node(),
                c"auto".to_node(),
                t.to_node(),
                l.to_node(),
            ]),
            (None, Some(_)) => panic!("Given subtitle language, but missing title"),
        }
    }

    /// See `AddSelect`. "Select the subtitle. If a subtitle with the same file name was
    /// already added, that one is selected, instead of loading a duplicate entry.
    /// (In this case, title/language are ignored, and if the [sub] was changed since it was loaded,
    /// these changes won't be reflected.)".
    pub fn subtitle_add_cached(&self, path: &CStr) -> Result<()> {
        self.command(&[c"sub-add".to_node(), path.to_node(), c"cached".to_node()])
    }

    /// "Remove the given subtitle track. If the id argument is missing, remove the current
    /// track. (Works on external subtitle files only.)"
    pub fn subtitle_remove(&self, index: Option<i64>) -> Result<()> {
        if let Some(idx) = index {
            self.command(&[c"sub-remove".to_node(), idx.to_node()])
        } else {
            self.command(&[c"sub-remove".to_node()])
        }
    }

    /// "Reload the given subtitle track. If the id argument is missing, reload the current
    /// track. (Works on external subtitle files only.)"
    pub fn subtitle_reload(&self, index: Option<i64>) -> Result<()> {
        if let Some(idx) = index {
            self.command(&[c"sub-reload".to_node(), idx.to_node()])
        } else {
            self.command(&[c"sub-reload".to_node()])
        }
    }

    /// "Change subtitle timing such, that the subtitle event after the next `isize` subtitle
    /// events is displayed. `isize` can be negative to step backwards."
    pub fn subtitle_step(&self, skip: i64) -> Result<()> {
        self.command(&[c"sub-step".to_node(), skip.to_node()])
    }

    /// "Seek to the next subtitle. This is similar to sub-step, except that it seeks video and
    /// audio instead of adjusting the subtitle delay.
    /// For embedded subtitles (like with matroska), this works only with subtitle events that
    /// have already been displayed, or are within a short prefetch range."
    pub fn subtitle_seek_forward(&self) -> Result<()> {
        self.command(&[c"sub-seek".to_node(), 1i64.to_node()])
    }

    /// See `SeekForward`.
    pub fn subtitle_seek_backward(&self) -> Result<()> {
        self.command(&[c"sub-seek".to_node(), (-1i64).to_node()])
    }
}
