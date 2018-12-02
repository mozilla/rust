//! The underlying OsString/OsStr implementation on Unix and many other
//! systems: just a `Vec<u8>`/`[u8]`.

use crate::borrow::Cow;
use crate::ffi::{OsStr, OsString};
use crate::fmt;
use crate::str;
use crate::mem;
use crate::ops::{Index, Range, RangeFrom, RangeTo};
use crate::rc::Rc;
use crate::sync::Arc;
use crate::sys_common::{FromInner, IntoInner, AsInner};
use crate::sys_common::bytestring::debug_fmt_bytestring;
use core::str::lossy::Utf8Lossy;
use core::slice::needles::{SliceSearcher, NaiveSearcher};
use needle::{Hay, Span, Searcher, ReverseSearcher, Consumer, ReverseConsumer};

#[derive(Clone, Hash)]
pub(crate) struct Buf {
    pub inner: Vec<u8>
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Slice {
    pub inner: [u8]
}

impl fmt::Debug for Slice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_fmt_bytestring(&self.inner, formatter)
    }
}

impl fmt::Display for Slice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&Utf8Lossy::from_bytes(&self.inner), formatter)
    }
}

impl Index<Range<usize>> for Slice {
    type Output = Slice;

    fn index(&self, range: Range<usize>) -> &Slice {
        Slice::from_u8_slice(&self.inner[range])
    }
}

impl Index<RangeFrom<usize>> for Slice {
    type Output = Slice;

    fn index(&self, range: RangeFrom<usize>) -> &Slice {
        Slice::from_u8_slice(&self.inner[range])
    }
}

impl Index<RangeTo<usize>> for Slice {
    type Output = Slice;

    fn index(&self, range: RangeTo<usize>) -> &Slice {
        Slice::from_u8_slice(&self.inner[range])
    }
}

impl fmt::Debug for Buf {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), formatter)
    }
}

impl fmt::Display for Buf {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_slice(), formatter)
    }
}

impl IntoInner<Vec<u8>> for Buf {
    fn into_inner(self) -> Vec<u8> {
        self.inner
    }
}

impl AsInner<[u8]> for Buf {
    fn as_inner(&self) -> &[u8] {
        &self.inner
    }
}


impl Buf {
    pub fn from_string(s: String) -> Buf {
        Buf { inner: s.into_bytes() }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Buf {
        Buf {
            inner: Vec::with_capacity(capacity)
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }

    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.inner.reserve_exact(additional)
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }

    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.inner.shrink_to(min_capacity)
    }

    pub fn as_slice(&self) -> &Slice {
        unsafe { mem::transmute(&*self.inner) }
    }

    pub fn into_string(self) -> Result<String, Buf> {
        String::from_utf8(self.inner).map_err(|p| Buf { inner: p.into_bytes() } )
    }

    pub fn push_slice(&mut self, s: &Slice) {
        self.inner.extend_from_slice(&s.inner)
    }

    #[inline]
    pub fn into_box(self) -> Box<Slice> {
        unsafe { mem::transmute(self.inner.into_boxed_slice()) }
    }

    #[inline]
    pub fn from_box(boxed: Box<Slice>) -> Buf {
        let inner: Box<[u8]> = unsafe { mem::transmute(boxed) };
        Buf { inner: inner.into_vec() }
    }

    #[inline]
    pub fn into_arc(&self) -> Arc<Slice> {
        self.as_slice().into_arc()
    }

    #[inline]
    pub fn into_rc(&self) -> Rc<Slice> {
        self.as_slice().into_rc()
    }
}

impl Slice {
    fn from_u8_slice(s: &[u8]) -> &Slice {
        unsafe { mem::transmute(s) }
    }

    pub fn from_str(s: &str) -> &Slice {
        Slice::from_u8_slice(s.as_bytes())
    }

    pub fn to_str(&self) -> Option<&str> {
        str::from_utf8(&self.inner).ok()
    }

    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.inner)
    }

    pub fn to_owned(&self) -> Buf {
        Buf { inner: self.inner.to_vec() }
    }

    #[inline]
    pub fn into_box(&self) -> Box<Slice> {
        let boxed: Box<[u8]> = self.inner.into();
        unsafe { mem::transmute(boxed) }
    }

    pub fn empty_box() -> Box<Slice> {
        let boxed: Box<[u8]> = Default::default();
        unsafe { mem::transmute(boxed) }
    }

    #[inline]
    pub fn into_arc(&self) -> Arc<Slice> {
        let arc: Arc<[u8]> = Arc::from(&self.inner);
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const Slice) }
    }

    #[inline]
    pub fn into_rc(&self) -> Rc<Slice> {
        let rc: Rc<[u8]> = Rc::from(&self.inner);
        unsafe { Rc::from_raw(Rc::into_raw(rc) as *const Slice) }
    }

    pub unsafe fn next_index(&self, index: usize) -> usize {
        self.inner.next_index(index)
    }

    pub unsafe fn prev_index(&self, index: usize) -> usize {
        self.inner.prev_index(index)
    }

    pub fn into_searcher(&self) -> OsStrSearcher<SliceSearcher<'_, u8>> {
        OsStrSearcher(SliceSearcher::new(&self.inner))
    }

    pub fn into_consumer(&self) -> OsStrSearcher<NaiveSearcher<'_, u8>> {
        OsStrSearcher(NaiveSearcher::new(&self.inner))
    }

    pub fn as_bytes_for_searcher(&self) -> &[u8] {
        &self.inner
    }
}

#[unstable(feature = "needle", issue = "56345")]
pub struct OsStrSearcher<S>(S);

#[unstable(feature = "needle", issue = "56345")]
unsafe impl<'p> Searcher<[u8]> for OsStrSearcher<SliceSearcher<'p, u8>> {
    #[inline]
    fn search(&mut self, span: Span<&[u8]>) -> Option<Range<usize>> {
        self.0.search(span)
    }
}

#[unstable(feature = "needle", issue = "56345")]
unsafe impl<'p> Consumer<[u8]> for OsStrSearcher<NaiveSearcher<'p, u8>> {
    #[inline]
    fn consume(&mut self, span: Span<&[u8]>) -> Option<usize> {
        self.0.consume(span)
    }

    #[inline]
    fn trim_start(&mut self, hay: &[u8]) -> usize {
        self.0.trim_start(hay)
    }
}

#[unstable(feature = "needle", issue = "56345")]
unsafe impl<'p> ReverseSearcher<[u8]> for OsStrSearcher<SliceSearcher<'p, u8>> {
    #[inline]
    fn rsearch(&mut self, span: Span<&[u8]>) -> Option<Range<usize>> {
        self.0.rsearch(span)
    }
}

#[unstable(feature = "needle", issue = "56345")]
unsafe impl<'p> ReverseConsumer<[u8]> for OsStrSearcher<NaiveSearcher<'p, u8>> {
    #[inline]
    fn rconsume(&mut self, span: Span<&[u8]>) -> Option<usize> {
        self.0.rconsume(span)
    }

    #[inline]
    fn trim_end(&mut self, hay: &[u8]) -> usize {
        self.0.trim_end(hay)
    }
}

/// Platform-specific extensions to [`OsString`].
///
/// [`OsString`]: ../../../../std/ffi/struct.OsString.html
#[stable(feature = "rust1", since = "1.0.0")]
pub trait OsStringExt {
    /// Creates an [`OsString`] from a byte vector.
    ///
    /// See the module docmentation for an example.
    ///
    /// [`OsString`]: ../../../ffi/struct.OsString.html
    #[stable(feature = "rust1", since = "1.0.0")]
    fn from_vec(vec: Vec<u8>) -> Self;

    /// Yields the underlying byte vector of this [`OsString`].
    ///
    /// See the module docmentation for an example.
    ///
    /// [`OsString`]: ../../../ffi/struct.OsString.html
    #[stable(feature = "rust1", since = "1.0.0")]
    fn into_vec(self) -> Vec<u8>;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl OsStringExt for OsString {
    fn from_vec(vec: Vec<u8>) -> OsString {
        FromInner::from_inner(Buf { inner: vec })
    }
    fn into_vec(self) -> Vec<u8> {
        self.into_inner().inner
    }
}

/// Platform-specific extensions to [`OsStr`].
///
/// [`OsStr`]: ../../../../std/ffi/struct.OsStr.html
#[stable(feature = "rust1", since = "1.0.0")]
pub trait OsStrExt {
    #[stable(feature = "rust1", since = "1.0.0")]
    /// Creates an [`OsStr`] from a byte slice.
    ///
    /// See the module docmentation for an example.
    ///
    /// [`OsStr`]: ../../../ffi/struct.OsStr.html
    fn from_bytes(slice: &[u8]) -> &Self;

    /// Gets the underlying byte view of the [`OsStr`] slice.
    ///
    /// See the module docmentation for an example.
    ///
    /// [`OsStr`]: ../../../ffi/struct.OsStr.html
    #[stable(feature = "rust1", since = "1.0.0")]
    fn as_bytes(&self) -> &[u8];
}

#[stable(feature = "rust1", since = "1.0.0")]
impl OsStrExt for OsStr {
    #[inline]
    fn from_bytes(slice: &[u8]) -> &OsStr {
        unsafe { mem::transmute(slice) }
    }
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &self.as_inner().inner
    }
}
