use std::{
    convert::TryInto,
    ffi::{CStr, c_char, c_void},
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::null_mut,
};

use crate::{mpv_error, mpv_format};

use super::{Format, GetData, Result, errors::Error};

#[derive(Debug)]
pub enum MpvNodeValue<'a> {
    String(&'a str),
    Flag(bool),
    Int64(i64),
    Double(f64),
    Array(MpvNodeArrayIter<'a>),
    Map(MpvNodeMapIter<'a>),
    None,
}

#[derive(Debug)]
pub struct MpvNodeArrayIter<'parent> {
    curr: i32,
    list: libmpv_sys::mpv_node_list,
    _does_not_outlive: PhantomData<&'parent MpvNode>,
}

impl Iterator for MpvNodeArrayIter<'_> {
    type Item = MpvNode;

    fn next(&mut self) -> Option<MpvNode> {
        if self.curr >= self.list.num {
            None
        } else {
            let offset = self.curr.try_into().ok()?;
            self.curr += 1;
            Some(MpvNode(unsafe { *self.list.values.offset(offset) }))
        }
    }
}

#[derive(Debug)]
pub struct MpvNodeMapIter<'parent> {
    curr: i32,
    list: libmpv_sys::mpv_node_list,
    _does_not_outlive: PhantomData<&'parent MpvNode>,
}

impl<'parent> Iterator for MpvNodeMapIter<'parent> {
    type Item = (&'parent str, MpvNode);

    fn next(&mut self) -> Option<(&'parent str, MpvNode)> {
        if self.curr >= self.list.num {
            None
        } else {
            let offset = self.curr.try_into().ok()?;
            let (key, value) = unsafe {
                (
                    mpv_cstr_to_str!(*self.list.keys.offset(offset)),
                    *self.list.values.offset(offset),
                )
            };
            self.curr += 1;
            Some((key.ok()?, MpvNode(value)))
        }
    }
}

#[derive(Debug)]
pub struct MpvNode(libmpv_sys::mpv_node);

impl Drop for MpvNode {
    fn drop(&mut self) {
        unsafe { libmpv_sys::mpv_free_node_contents(&mut self.0 as *mut libmpv_sys::mpv_node) };
    }
}

impl MpvNode {
    pub(crate) fn new(val: libmpv_sys::mpv_node) -> Self {
        MpvNode(val)
    }

    pub fn value(&self) -> Result<MpvNodeValue<'_>> {
        let node = self.0;
        Ok(match node.format {
            mpv_format::Flag => MpvNodeValue::Flag(unsafe { node.u.flag } == 1),
            mpv_format::Int64 => MpvNodeValue::Int64(unsafe { node.u.int64 }),
            mpv_format::Double => MpvNodeValue::Double(unsafe { node.u.double_ }),
            mpv_format::String => {
                let text = unsafe { mpv_cstr_to_str!(node.u.string) }?;
                MpvNodeValue::String(text)
            }

            mpv_format::Array => MpvNodeValue::Array(MpvNodeArrayIter {
                list: unsafe { *node.u.list },
                curr: 0,
                _does_not_outlive: PhantomData,
            }),

            mpv_format::Map => MpvNodeValue::Map(MpvNodeMapIter {
                list: unsafe { *node.u.list },
                curr: 0,
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

    pub fn to_str(&self) -> Option<&str> {
        if let MpvNodeValue::String(value) = self.value().ok()? {
            Some(value)
        } else {
            None
        }
    }

    pub fn to_array(&self) -> Option<MpvNodeArrayIter<'_>> {
        if let MpvNodeValue::Array(value) = self.value().ok()? {
            Some(value)
        } else {
            None
        }
    }

    pub fn to_map(&self) -> Option<MpvNodeMapIter<'_>> {
        if let MpvNodeValue::Map(value) = self.value().ok()? {
            Some(value)
        } else {
            None
        }
    }
}

unsafe impl GetData for MpvNode {
    fn get_from_c_void<T, F: FnMut(*mut c_void) -> Result<T>>(mut fun: F) -> Result<MpvNode> {
        let mut val = MaybeUninit::uninit();
        let _ = fun(val.as_mut_ptr() as *mut _)?;
        Ok(MpvNode(unsafe { val.assume_init() }))
    }

    fn get_format() -> Format {
        Format::Node
    }
}

#[repr(transparent)]
pub struct BorrowingMpvNode<'n> {
    node: libmpv_sys::mpv_node,
    _l: PhantomData<&'n libmpv_sys::mpv_node>,
}

impl BorrowingMpvNode<'_> {
    pub fn node(&self) -> *mut libmpv_sys::mpv_node {
        (&raw const self.node).cast_mut()
    }
}

pub trait ToNode<'n> {
    fn to_node(self) -> BorrowingMpvNode<'n>;
}

impl<'n> ToNode<'n> for &'n CStr {
    fn to_node(self) -> BorrowingMpvNode<'n> {
        BorrowingMpvNode {
            node: libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 {
                    string: self.as_ptr().cast_mut(),
                },
                format: libmpv_sys::mpv_format_MPV_FORMAT_STRING,
            },
            _l: PhantomData,
        }
    }
}

impl ToNode<'static> for i64 {
    fn to_node(self) -> BorrowingMpvNode<'static> {
        BorrowingMpvNode {
            node: libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 { int64: self },
                format: libmpv_sys::mpv_format_MPV_FORMAT_INT64,
            },
            _l: PhantomData,
        }
    }
}

impl ToNode<'static> for bool {
    fn to_node(self) -> BorrowingMpvNode<'static> {
        BorrowingMpvNode {
            node: libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 {
                    flag: if self { 1 } else { 0 },
                },
                format: libmpv_sys::mpv_format_MPV_FORMAT_FLAG,
            },
            _l: PhantomData,
        }
    }
}

impl ToNode<'static> for f64 {
    fn to_node(self) -> BorrowingMpvNode<'static> {
        BorrowingMpvNode {
            node: libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 { double_: self },
                format: libmpv_sys::mpv_format_MPV_FORMAT_DOUBLE,
            },
            _l: PhantomData,
        }
    }
}

#[repr(transparent)]
pub struct BorrowingMpvNodeList<'n> {
    list: libmpv_sys::mpv_node_list,
    _l: PhantomData<&'n libmpv_sys::mpv_node>,
}

impl<'n> BorrowingMpvNodeList<'n> {
    pub fn new(list: &'n [BorrowingMpvNode<'n>]) -> Self {
        BorrowingMpvNodeList {
            list: libmpv_sys::mpv_node_list {
                num: list.len().try_into().expect("length overflow"),
                values: list.as_ptr().cast_mut().cast(),
                keys: null_mut(),
            },
            _l: PhantomData,
        }
    }
}

impl<'n> ToNode<'n> for &'n BorrowingMpvNodeList<'n> {
    fn to_node(self) -> BorrowingMpvNode<'n> {
        BorrowingMpvNode {
            node: libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 {
                    list: (&raw const self.list).cast_mut(),
                },
                format: libmpv_sys::mpv_format_MPV_FORMAT_NODE_ARRAY,
            },
            _l: PhantomData,
        }
    }
}

#[repr(transparent)]
pub struct BorrowingMpvNodeMap<'n> {
    list: libmpv_sys::mpv_node_list,
    _l: PhantomData<&'n libmpv_sys::mpv_node>,
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

impl<'n> BorrowingMpvNodeMap<'n> {
    pub fn new(keys: &'n [BorrowingCPtr<'n>], values: &'n [BorrowingMpvNode<'n>]) -> Self {
        assert_eq!(
            keys.len(),
            values.len(),
            "keys and values have differing length"
        );
        BorrowingMpvNodeMap {
            list: libmpv_sys::mpv_node_list {
                num: keys.len().try_into().expect("length overflow"),
                values: values.as_ptr().cast_mut().cast(),
                keys: keys.as_ptr().cast_mut().cast(),
            },
            _l: PhantomData,
        }
    }
}

impl<'n> ToNode<'n> for &'n BorrowingMpvNodeMap<'n> {
    fn to_node(self) -> BorrowingMpvNode<'n> {
        BorrowingMpvNode {
            node: libmpv_sys::mpv_node {
                u: libmpv_sys::mpv_node__bindgen_ty_1 {
                    list: (&raw const self.list).cast_mut(),
                },
                format: libmpv_sys::mpv_format_MPV_FORMAT_NODE_MAP,
            },
            _l: PhantomData,
        }
    }
}
