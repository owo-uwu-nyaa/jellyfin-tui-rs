use std::{
    convert::TryInto,
    ffi::{CStr, c_char, c_void},
    fmt::Debug,
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::null_mut,
};

use crate::{mpv::mpv_cstr_to_str, mpv_error, mpv_format};

use super::{Format, GetData, Result, errors::Error};

pub enum MpvNodeValue<'a> {
    String(&'a str),
    Flag(bool),
    Int64(i64),
    Double(f64),
    Array(MpvNodeArrayRef<'a>),
    Map(MpvNodeMapRef<'a>),
    None,
}

impl<'a> std::fmt::Debug for MpvNodeValue<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(arg0) => f.debug_tuple("String").field(arg0).finish(),
            Self::Flag(arg0) => f.debug_tuple("Flag").field(arg0).finish(),
            Self::Int64(arg0) => f.debug_tuple("Int64").field(arg0).finish(),
            Self::Double(arg0) => f.debug_tuple("Double").field(arg0).finish(),
            Self::Array(arg0) => f.debug_tuple("Array").field(arg0).finish(),
            Self::Map(arg0) => f.debug_tuple("Map").field(arg0).finish(),
            Self::None => write!(f, "None"),
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct MpvNodeArrayRef<'parent> {
    list: libmpv_sys::mpv_node_list,
    _does_not_outlive: PhantomData<&'parent MpvNode>,
}

impl<'p> IntoIterator for MpvNodeArrayRef<'p> {
    type Item = MpvNodeRef<'p>;

    type IntoIter = MpvNodeArrayIter<'p>;

    fn into_iter(self) -> Self::IntoIter {
        MpvNodeArrayIter {
            curr: 0,
            list: self.list,
            _does_not_outlive: PhantomData,
        }
    }
}

impl Debug for MpvNodeArrayRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(*self).finish()
    }
}

pub struct MpvNodeArrayIter<'parent> {
    curr: i32,
    list: libmpv_sys::mpv_node_list,
    _does_not_outlive: PhantomData<&'parent MpvNode>,
}

impl<'p> Iterator for MpvNodeArrayIter<'p> {
    type Item = MpvNodeRef<'p>;

    fn next(&mut self) -> Option<MpvNodeRef<'p>> {
        if self.curr >= self.list.num {
            None
        } else {
            let offset = self.curr.try_into().ok()?;
            self.curr += 1;
            Some(unsafe { MpvNodeRef::new(*self.list.values.offset(offset)) })
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct MpvNodeMapRef<'parent> {
    list: libmpv_sys::mpv_node_list,
    _does_not_outlive: PhantomData<&'parent MpvNode>,
}

impl<'p> IntoIterator for MpvNodeMapRef<'p> {
    type Item = (&'p CStr, MpvNodeRef<'p>);

    type IntoIter = MpvNodeMapIter<'p>;

    fn into_iter(self) -> Self::IntoIter {
        MpvNodeMapIter {
            curr: 0,
            list: self.list,
            _does_not_outlive: PhantomData,
        }
    }
}

impl Debug for MpvNodeMapRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(*self).finish()
    }
}

pub struct MpvNodeMapIter<'parent> {
    curr: i32,
    list: libmpv_sys::mpv_node_list,
    _does_not_outlive: PhantomData<&'parent MpvNode>,
}

impl<'p> Iterator for MpvNodeMapIter<'p> {
    type Item = (&'p CStr, MpvNodeRef<'p>);

    fn next(&mut self) -> Option<(&'p CStr, MpvNodeRef<'p>)> {
        if self.curr >= self.list.num {
            None
        } else {
            let offset = self
                .curr
                .try_into()
                .expect("error converting index to offset");
            self.curr += 1;
            Some(unsafe {
                (
                    CStr::from_ptr(*self.list.keys.offset(offset)),
                    MpvNodeRef::new(*self.list.values.offset(offset)),
                )
            })
        }
    }
}

#[repr(transparent)]
pub struct MpvNodeRef<'p> {
    node: libmpv_sys::mpv_node,
    _does_not_outlive: PhantomData<&'p MpvNode>,
}

impl Debug for MpvNodeRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.value() {
            Ok(v) => Debug::fmt(&v, f),
            Err(_) => f.write_str("(Unknown property kind)"),
        }
    }
}

impl<'p> MpvNodeRef<'p> {
    pub(crate) unsafe fn new<'s>(val: libmpv_sys::mpv_node) -> MpvNodeRef<'s> {
        MpvNodeRef {
            node: val,
            _does_not_outlive: PhantomData,
        }
    }

    pub fn node(&self) -> *mut libmpv_sys::mpv_node {
        (&raw const self.node).cast_mut()
    }

    pub fn value(&self) -> Result<MpvNodeValue<'p>> {
        Ok(match self.node.format {
            mpv_format::Flag => MpvNodeValue::Flag(unsafe { self.node.u.flag } == 1),
            mpv_format::Int64 => MpvNodeValue::Int64(unsafe { self.node.u.int64 }),
            mpv_format::Double => MpvNodeValue::Double(unsafe { self.node.u.double_ }),
            mpv_format::String => {
                MpvNodeValue::String(unsafe { mpv_cstr_to_str(self.node.u.string) }?)
            }

            mpv_format::Array => MpvNodeValue::Array(MpvNodeArrayRef {
                list: unsafe { *self.node.u.list },
                _does_not_outlive: PhantomData,
            }),

            mpv_format::Map => MpvNodeValue::Map(MpvNodeMapRef {
                list: unsafe { *self.node.u.list },
                _does_not_outlive: PhantomData,
            }),
            mpv_format::None => MpvNodeValue::None,
            _ => return Err(Error::Raw(mpv_error::PropertyError)),
        })
    }

    pub fn to_bool(&self) -> Option<bool> {
        if let MpvNodeValue::Flag(value) = self.value().ok()? {
            Some(value)
        } else {
            None
        }
    }
    pub fn to_i64(&self) -> Option<i64> {
        if let MpvNodeValue::Int64(value) = self.value().ok()? {
            Some(value)
        } else {
            None
        }
    }
    pub fn to_f64(&self) -> Option<f64> {
        if let MpvNodeValue::Double(value) = self.value().ok()? {
            Some(value)
        } else {
            None
        }
    }

    pub fn to_str(&self) -> Option<&'p str> {
        if let MpvNodeValue::String(value) = self.value().ok()? {
            Some(value)
        } else {
            None
        }
    }

    pub fn to_array(&self) -> Option<MpvNodeArrayRef<'p>> {
        if let MpvNodeValue::Array(value) = self.value().ok()? {
            Some(value)
        } else {
            None
        }
    }

    pub fn to_map(&self) -> Option<MpvNodeMapRef<'p>> {
        if let MpvNodeValue::Map(value) = self.value().ok()? {
            Some(value)
        } else {
            None
        }
    }
}

#[repr(transparent)]
pub struct MpvNode {
    node: libmpv_sys::mpv_node,
}

impl Drop for MpvNode {
    fn drop(&mut self) {
        unsafe { libmpv_sys::mpv_free_node_contents(&mut self.node) };
    }
}

impl MpvNode {
    pub(crate) unsafe fn new(val: libmpv_sys::mpv_node) -> Self {
        MpvNode { node: val }
    }
    pub fn as_ref<'s>(&'s self) -> MpvNodeRef<'s> {
        MpvNodeRef {
            node: self.node,
            _does_not_outlive: PhantomData,
        }
    }
}

impl Debug for MpvNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.as_ref(), f)
    }
}

unsafe impl GetData for MpvNode {
    unsafe fn get_from_c_void<T, F: FnMut(*mut c_void) -> Result<T>>(
        mut fun: F,
    ) -> Result<MpvNode> {
        let mut val = MaybeUninit::uninit();
        let _ = fun(val.as_mut_ptr() as *mut _)?;
        Ok(MpvNode {
            node: unsafe { val.assume_init() },
        })
    }

    fn get_format() -> Format {
        Format::Node
    }
}

pub trait ToNode<'n> {
    fn to_node(self) -> MpvNodeRef<'n>;
}

impl<'n> ToNode<'n> for &'n CStr {
    fn to_node(self) -> MpvNodeRef<'n> {
        unsafe {
            MpvNodeRef::new(libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 {
                    string: self.as_ptr().cast_mut(),
                },
                format: libmpv_sys::mpv_format_MPV_FORMAT_STRING,
            })
        }
    }
}

impl ToNode<'static> for i64 {
    fn to_node(self) -> MpvNodeRef<'static> {
        unsafe {
            MpvNodeRef::new(libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 { int64: self },
                format: libmpv_sys::mpv_format_MPV_FORMAT_INT64,
            })
        }
    }
}

impl ToNode<'static> for bool {
    fn to_node(self) -> MpvNodeRef<'static> {
        unsafe {
            MpvNodeRef::new(libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 {
                    flag: if self { 1 } else { 0 },
                },
                format: libmpv_sys::mpv_format_MPV_FORMAT_FLAG,
            })
        }
    }
}

impl ToNode<'static> for f64 {
    fn to_node(self) -> MpvNodeRef<'static> {
        unsafe {
            MpvNodeRef::new(libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 { double_: self },
                format: libmpv_sys::mpv_format_MPV_FORMAT_DOUBLE,
            })
        }
    }
}

impl<'n> MpvNodeArrayRef<'n> {
    pub fn new(list: &'n [MpvNodeRef<'n>]) -> Self {
        MpvNodeArrayRef {
            list: libmpv_sys::mpv_node_list {
                num: list.len().try_into().expect("length overflow"),
                values: list.as_ptr().cast_mut().cast(),
                keys: null_mut(),
            },
            _does_not_outlive: PhantomData,
        }
    }
}

impl<'n> ToNode<'n> for &'n MpvNodeArrayRef<'n> {
    fn to_node(self) -> MpvNodeRef<'n> {
        unsafe {
            MpvNodeRef::new(libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 {
                    list: (&raw const self.list).cast_mut(),
                },
                format: libmpv_sys::mpv_format_MPV_FORMAT_NODE_ARRAY,
            })
        }
    }
}

#[repr(transparent)]
pub struct BorrowingCPtr<'n> {
    ptr: *mut c_char,
    _l: PhantomData<&'n CStr>,
}

impl<'n> BorrowingCPtr<'n> {
    pub fn new(s: &'n CStr) -> Self {
        BorrowingCPtr {
            ptr: s.as_ptr().cast_mut(),
            _l: PhantomData,
        }
    }
}

impl<'n> MpvNodeMapRef<'n> {
    pub fn new(keys: &'n [BorrowingCPtr<'n>], values: &'n [MpvNodeRef<'n>]) -> Self {
        assert_eq!(
            keys.len(),
            values.len(),
            "keys and values have differing length"
        );
        MpvNodeMapRef {
            list: libmpv_sys::mpv_node_list {
                num: keys.len().try_into().expect("length overflow"),
                values: values.as_ptr().cast_mut().cast(),
                keys: keys.as_ptr().cast_mut().cast(),
            },
            _does_not_outlive: PhantomData,
        }
    }
}

impl<'n> ToNode<'n> for &'n MpvNodeMapRef<'n> {
    fn to_node(self) -> MpvNodeRef<'n> {
        unsafe {
            MpvNodeRef::new(libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 {
                    list: (&raw const self.list).cast_mut(),
                },
                format: libmpv_sys::mpv_format_MPV_FORMAT_NODE_MAP,
            })
        }
    }
}
