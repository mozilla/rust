//! Implementations of things like `Eq` for fixed-length arrays
//! up to a certain length. Eventually we should able to generalize
//! to all lengths.
//!
//! *[See also the array primitive type](../../std/primitive.array.html).*

#![stable(feature = "core_array", since = "1.36.0")]

use crate::borrow::{Borrow, BorrowMut};
use crate::cmp::Ordering;
use crate::convert::{Infallible, TryFrom};
use crate::fmt;
use crate::hash::{Hash, self};
use crate::marker::Unsize;
use crate::slice::{Iter, IterMut};

/// Utility trait implemented only on arrays of fixed size
///
/// This trait can be used to implement other traits on fixed-size arrays
/// without causing much metadata bloat.
///
/// The trait is marked unsafe in order to restrict implementors to fixed-size
/// arrays. User of this trait can assume that implementors have the exact
/// layout in memory of a fixed size array (for example, for unsafe
/// initialization).
///
/// Note that the traits AsRef and AsMut provide similar methods for types that
/// may not be fixed-size arrays. Implementors should prefer those traits
/// instead.
#[unstable(feature = "fixed_size_array", issue = "27778")]
pub unsafe trait FixedSizeArray<T> {
    /// Converts the array to immutable slice
    #[unstable(feature = "fixed_size_array", issue = "27778")]
    fn as_slice(&self) -> &[T];
    /// Converts the array to mutable slice
    #[unstable(feature = "fixed_size_array", issue = "27778")]
    fn as_mut_slice(&mut self) -> &mut [T];
}

#[unstable(feature = "fixed_size_array", issue = "27778")]
unsafe impl<T, A: Unsize<[T]>> FixedSizeArray<T> for A {
    #[inline]
    fn as_slice(&self) -> &[T] {
        self
    }
    #[inline]
    fn as_mut_slice(&mut self) -> &mut [T] {
        self
    }
}

/// The error type returned when a conversion from a slice to an array fails.
#[stable(feature = "try_from", since = "1.34.0")]
#[derive(Debug, Copy, Clone)]
pub struct TryFromSliceError(());

#[stable(feature = "core_array", since = "1.36.0")]
impl fmt::Display for TryFromSliceError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.__description(), f)
    }
}

impl TryFromSliceError {
    #[unstable(feature = "array_error_internals",
           reason = "available through Error trait and this method should not \
                     be exposed publicly",
           issue = "0")]
    #[inline]
    #[doc(hidden)]
    pub fn __description(&self) -> &str {
        "could not convert slice to array"
    }
}

#[stable(feature = "try_from_slice_error", since = "1.36.0")]
impl From<Infallible> for TryFromSliceError {
    fn from(x: Infallible) -> TryFromSliceError {
        match x {}
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, const N: usize> AsRef<[T]> for [T; N] {
    #[inline]
    fn as_ref(&self) -> &[T] {
        &self[..]
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, const N: usize> AsMut<[T]> for [T; N] {
    #[inline]
    fn as_mut(&mut self) -> &mut [T] {
        &mut self[..]
    }
}

#[stable(feature = "array_borrow", since = "1.4.0")]
impl<T, const N: usize> Borrow<[T]> for [T; N] {
    fn borrow(&self) -> &[T] {
        self
    }
}

#[stable(feature = "array_borrow", since = "1.4.0")]
impl<T, const N: usize> BorrowMut<[T]> for [T; N] {
    fn borrow_mut(&mut self) -> &mut [T] {
        self
    }
}

#[stable(feature = "try_from", since = "1.34.0")]
impl<T, const N: usize> TryFrom<&[T]> for [T; N] where T: Copy {
    type Error = TryFromSliceError;

    fn try_from(slice: &[T]) -> Result<[T; N], TryFromSliceError> {
        <&Self>::try_from(slice).map(|r| *r)
    }
}

#[stable(feature = "try_from", since = "1.34.0")]
impl<'a, T, const N: usize> TryFrom<&'a [T]> for &'a [T; N] {
    type Error = TryFromSliceError;

    fn try_from(slice: &[T]) -> Result<&[T; N], TryFromSliceError> {
        if slice.len() == N {
            let ptr = slice.as_ptr() as *const [T; N];
            unsafe { Ok(&*ptr) }
        } else {
            Err(TryFromSliceError(()))
        }
    }
}

#[stable(feature = "try_from", since = "1.34.0")]
impl<'a, T, const N: usize> TryFrom<&'a mut [T]> for &'a mut [T; N] {
    type Error = TryFromSliceError;

    fn try_from(slice: &mut [T]) -> Result<&mut [T; N], TryFromSliceError> {
        if slice.len() == N {
            let ptr = slice.as_mut_ptr() as *mut [T; N];
            unsafe { Ok(&mut *ptr) }
        } else {
            Err(TryFromSliceError(()))
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Hash, const N: usize> Hash for [T; N] {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        Hash::hash(&self[..], state)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: fmt::Debug, const N: usize> fmt::Debug for [T; N] {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&&self[..], f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, const N: usize> IntoIterator for &'a [T; N] {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, const N: usize> IntoIterator for &'a mut [T; N] {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<[B; N]> for [A; N] where A: PartialEq<B> {
    #[inline]
    fn eq(&self, other: &[B; N]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &[B; N]) -> bool { self[..] != other[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<[B]> for [A; N] where A: PartialEq<B> {
    #[inline]
    fn eq(&self, other: &[B]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &[B]) -> bool { self[..] != other[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<[A; N]> for [B] where B: PartialEq<A> {
    #[inline]
    fn eq(&self, other: &[A; N]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &[A; N]) -> bool { self[..] != other[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<&'b [B]> for [A; N] where A: PartialEq<B> {
    #[inline]
    fn eq(&self, other: &&'b [B]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &&'b [B]) -> bool { self[..] != other[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<[A; N]> for &'b [B] where B: PartialEq<A> {
    #[inline]
    fn eq(&self, other: &[A; N]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &[A; N]) -> bool { self[..] != other[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<&'b mut [B]> for [A; N] where A: PartialEq<B> {
    #[inline]
    fn eq(&self, other: &&'b mut [B]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &&'b mut [B]) -> bool { self[..] != other[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<[A; N]> for &'b mut [B] where B: PartialEq<A> {
    #[inline]
    fn eq(&self, other: &[A; N]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &[A; N]) -> bool { self[..] != other[..] }
}

// NOTE: some less important impls are omitted to reduce code bloat
// after const generics came true in 2019, we didn't immediate add these back.

/*
#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<&'b [B; N]> for [A; N] where A: PartialEq<B> {
    #[inline]
    fn eq(&self, other: &&'b [B; N]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &&'b [B; N]) -> bool { self[..] != other[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<[A; N]> for &'b [B; N] where B: PartialEq<A> {
    #[inline]
    fn eq(&self, other: &[A; N]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &[A; N]) -> bool { self[..] != other[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<&'b mut [B; N]> for [A; N]
    where A: PartialEq<B> {
    #[inline]
    fn eq(&self, other: &&'b mut [B; N]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &&'b mut [B; N]) -> bool { self[..] != other[..] }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, 'b, A: Sized, B, const N: usize> PartialEq<[A; N]> for &'b mut [B; N]
    where B: PartialEq<A> {
    #[inline]
    fn eq(&self, other: &[A; N]) -> bool { self[..] == other[..] }
    #[inline]
    fn ne(&self, other: &[A; N]) -> bool { self[..] != other[..] }
}

*/

#[stable(feature = "rust1", since = "1.0.0")]
impl<T:Eq, const N: usize> Eq for [T; N] { }

#[stable(feature = "rust1", since = "1.0.0")]
impl<T:PartialOrd, const N: usize> PartialOrd for [T; N] {
    #[inline]
    fn partial_cmp(&self, other: &[T; N]) -> Option<Ordering> {
        PartialOrd::partial_cmp(&&self[..], &&other[..])
    }
    #[inline]
    fn lt(&self, other: &[T; N]) -> bool {
        PartialOrd::lt(&&self[..], &&other[..])
    }
    #[inline]
    fn le(&self, other: &[T; N]) -> bool {
        PartialOrd::le(&&self[..], &&other[..])
    }
    #[inline]
    fn ge(&self, other: &[T; N]) -> bool {
        PartialOrd::ge(&&self[..], &&other[..])
    }
    #[inline]
    fn gt(&self, other: &[T; N]) -> bool {
        PartialOrd::gt(&&self[..], &&other[..])
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T:Ord, const N: usize> Ord for [T; N] {
    #[inline]
    fn cmp(&self, other: &[T; N]) -> Ordering {
        Ord::cmp(&&self[..], &&other[..])
    }
}

#[stable(since = "1.4.0", feature = "array_default")]
impl<T, const N: usize> Default for [T; N] where T: Default {
    fn default() -> [T; N] {
        [T::default(); N]
    }
}
