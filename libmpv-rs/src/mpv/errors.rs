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

use std::{error, ffi::NulError, fmt, num::TryFromIntError, os::raw as ctype, str::Utf8Error};

#[allow(missing_docs)]
pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Loadfiles {
        index: usize,
        error: Box<Error>,
    },
    VersionMismatch {
        linked: ctype::c_ulong,
        loaded: ctype::c_ulong,
    },
    InvalidUtf8,
    Null,
    Raw(crate::MpvError),
    IntConversion(TryFromIntError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

impl From<NulError> for Error {
    fn from(_other: NulError) -> Error {
        Error::Null
    }
}

impl From<Utf8Error> for Error {
    fn from(_other: Utf8Error) -> Error {
        Error::InvalidUtf8
    }
}
impl From<crate::MpvError> for Error {
    fn from(other: crate::MpvError) -> Error {
        Error::Raw(other)
    }
}

impl From<TryFromIntError> for Error {
    fn from(value: TryFromIntError) -> Self {
        Error::IntConversion(value)
    }
}

impl error::Error for Error {}
