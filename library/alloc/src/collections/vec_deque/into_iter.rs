use core::fmt;
use core::iter::{FusedIterator, TrustedLen, TrustedRandomAccess};

use crate::alloc::{Allocator, Global};

use super::VecDeque;

/// An owning iterator over the elements of a `VecDeque`.
///
/// This `struct` is created by the [`into_iter`] method on [`VecDeque`]
/// (provided by the `IntoIterator` trait). See its documentation for more.
///
/// [`into_iter`]: VecDeque::into_iter
#[derive(Clone)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct IntoIter<T, A: Allocator = Global> {
    pub(crate) inner: VecDeque<T, A>,
}

#[stable(feature = "collection_debug", since = "1.17.0")]
impl<T: fmt::Debug, A: Allocator> fmt::Debug for IntoIter<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.inner).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, A: Allocator> Iterator for IntoIter<T, A> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.inner.pop_front()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.inner.len();
        (len, Some(len))
    }

    #[inline]
    #[doc(hidden)]
    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item
    where
        Self: TrustedRandomAccess,
    {
        // Safety: The TrustedRandomAccess contract requires that callers only pass an index
        // that is in bounds.
        // Additionally Self: TrustedRandomAccess is only implemented for T: Copy which means even
        // multiple repeated reads of the same index would be safe and the
        // values are !Drop, thus won't suffer from double drops.
        unsafe {
            let idx = self.inner.wrap_add(self.inner.tail, idx);
            self.inner.buffer_read(idx)
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, A: Allocator> DoubleEndedIterator for IntoIter<T, A> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        self.inner.pop_back()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, A: Allocator> ExactSizeIterator for IntoIter<T, A> {
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<T, A: Allocator> FusedIterator for IntoIter<T, A> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T, A: Allocator> TrustedLen for IntoIter<T, A> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
// T: Copy as approximation for !Drop since get_unchecked does not update the pointers
// and thus we can't implement drop-handling
unsafe impl<T, A: Allocator> TrustedRandomAccess for IntoIter<T, A>
where
    T: Copy,
{
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}
