use super::{From, TryFrom};
use crate::num::TryFromIntError;

mod private {
    /// This trait being unreachable from outside the crate
    /// prevents other implementations of the `FloatToInt` trait,
    /// which allows potentially adding more trait methods after the trait is `#[stable]`.
    #[unstable(feature = "convert_float_to_int", issue = "67057")]
    pub trait Sealed {}
}

/// Supporting trait for inherent methods of `f32` and `f64` such as `round_unchecked_to`.
/// Typically doesn’t need to be used directly.
#[unstable(feature = "convert_float_to_int", issue = "67057")]
pub trait FloatToInt<Int>: private::Sealed + Sized {
    #[unstable(feature = "convert_float_to_int", issue = "67057")]
    #[doc(hidden)]
    unsafe fn to_int_unchecked(self) -> Int;
}

macro_rules! impl_float_to_int {
    ( $Float: ident => $( $Int: ident )+ ) => {
        #[unstable(feature = "convert_float_to_int", issue = "67057")]
        impl private::Sealed for $Float {}
        $(
            #[unstable(feature = "convert_float_to_int", issue = "67057")]
            impl FloatToInt<$Int> for $Float {
                #[doc(hidden)]
                #[inline]
                unsafe fn to_int_unchecked(self) -> $Int {
                    crate::intrinsics::float_to_int_unchecked(self)
                }
            }
        )+
    }
}

impl_float_to_int!(f32 => u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize);
impl_float_to_int!(f64 => u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize);

// Conversion traits for primitive integer and float types
// Conversions T -> T are covered by a blanket impl and therefore excluded
// Some conversions from and to usize/isize are not implemented due to portability concerns
macro_rules! impl_from {
    ($Small: ty, $Large: ty, #[$attr:meta], $doc: expr) => {
        #[$attr]
        #[doc = $doc]
        impl From<$Small> for $Large {
            #[inline]
            fn from(small: $Small) -> Self {
                small as Self
            }
        }
    };
    ($Small: ty, $Large: ty, #[$attr:meta]) => {
        impl_from!($Small,
                   $Large,
                   #[$attr],
                   concat!("Converts `",
                           stringify!($Small),
                           "` to `",
                           stringify!($Large),
                           "` losslessly."));
    }
}

macro_rules! impl_from_bool {
    ($target: ty, #[$attr:meta]) => {
        impl_from!(bool, $target, #[$attr], concat!("Converts a `bool` to a `",
            stringify!($target), "`. The resulting value is `0` for `false` and `1` for `true`
values.

# Examples

```
assert_eq!(", stringify!($target), "::from(true), 1);
assert_eq!(", stringify!($target), "::from(false), 0);
```"));
    };
}

// Bool -> Any
impl_from_bool! { u8, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { u16, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { u32, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { u64, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { u128, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { usize, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { i8, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { i16, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { i32, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { i64, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { i128, #[stable(feature = "from_bool", since = "1.28.0")] }
impl_from_bool! { isize, #[stable(feature = "from_bool", since = "1.28.0")] }

// Unsigned -> Unsigned
impl_from! { u8, u16, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u8, u32, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u8, u64, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u8, u128, #[stable(feature = "i128", since = "1.26.0")] }
impl_from! { u8, usize, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u16, u32, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u16, u64, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u16, u128, #[stable(feature = "i128", since = "1.26.0")] }
impl_from! { u32, u64, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u32, u128, #[stable(feature = "i128", since = "1.26.0")] }
impl_from! { u64, u128, #[stable(feature = "i128", since = "1.26.0")] }

// Signed -> Signed
impl_from! { i8, i16, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { i8, i32, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { i8, i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { i8, i128, #[stable(feature = "i128", since = "1.26.0")] }
impl_from! { i8, isize, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { i16, i32, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { i16, i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { i16, i128, #[stable(feature = "i128", since = "1.26.0")] }
impl_from! { i32, i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { i32, i128, #[stable(feature = "i128", since = "1.26.0")] }
impl_from! { i64, i128, #[stable(feature = "i128", since = "1.26.0")] }

// Unsigned -> Signed
impl_from! { u8, i16, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u8, i32, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u8, i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u8, i128, #[stable(feature = "i128", since = "1.26.0")] }
impl_from! { u16, i32, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u16, i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u16, i128, #[stable(feature = "i128", since = "1.26.0")] }
impl_from! { u32, i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")] }
impl_from! { u32, i128, #[stable(feature = "i128", since = "1.26.0")] }
impl_from! { u64, i128, #[stable(feature = "i128", since = "1.26.0")] }

// The C99 standard defines bounds on INTPTR_MIN, INTPTR_MAX, and UINTPTR_MAX
// which imply that pointer-sized integers must be at least 16 bits:
// https://port70.net/~nsz/c/c99/n1256.html#7.18.2.4
impl_from! { u16, usize, #[stable(feature = "lossless_iusize_conv", since = "1.26.0")] }
impl_from! { u8, isize, #[stable(feature = "lossless_iusize_conv", since = "1.26.0")] }
impl_from! { i16, isize, #[stable(feature = "lossless_iusize_conv", since = "1.26.0")] }

// RISC-V defines the possibility of a 128-bit address space (RV128).

// CHERI proposes 256-bit “capabilities”. Unclear if this would be relevant to usize/isize.
// https://www.cl.cam.ac.uk/research/security/ctsrd/pdfs/20171017a-cheri-poster.pdf
// http://www.csl.sri.com/users/neumann/2012resolve-cheri.pdf

// Note: integers can only be represented with full precision in a float if
// they fit in the significand, which is 24 bits in f32 and 53 bits in f64.
// Lossy float conversions are not implemented at this time.

// Signed -> Float
impl_from! { i8, f32, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }
impl_from! { i8, f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }
impl_from! { i16, f32, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }
impl_from! { i16, f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }
impl_from! { i32, f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }

// Unsigned -> Float
impl_from! { u8, f32, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }
impl_from! { u8, f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }
impl_from! { u16, f32, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }
impl_from! { u16, f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }
impl_from! { u32, f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }

// Float -> Float
impl_from! { f32, f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")] }

// no possible bounds violation
macro_rules! try_from_unbounded {
    ($source:ty, $($target:ty),*) => {$(
        #[stable(feature = "try_from", since = "1.34.0")]
        impl TryFrom<$source> for $target {
            type Error = TryFromIntError;

            /// Try to create the target number type from a source
            /// number type. This returns an error if the source value
            /// is outside of the range of the target type.
            #[inline]
            fn try_from(value: $source) -> Result<Self, Self::Error> {
                Ok(value as Self)
            }
        }
    )*}
}

// only negative bounds
macro_rules! try_from_lower_bounded {
    ($source:ty, $($target:ty),*) => {$(
        #[stable(feature = "try_from", since = "1.34.0")]
        impl TryFrom<$source> for $target {
            type Error = TryFromIntError;

            /// Try to create the target number type from a source
            /// number type. This returns an error if the source value
            /// is outside of the range of the target type.
            #[inline]
            fn try_from(u: $source) -> Result<Self, Self::Error> {
                if u >= 0 {
                    Ok(u as Self)
                } else {
                    Err(TryFromIntError(()))
                }
            }
        }
    )*}
}

// unsigned to signed (only positive bound)
macro_rules! try_from_upper_bounded {
    ($source:ty, $($target:ty),*) => {$(
        #[stable(feature = "try_from", since = "1.34.0")]
        impl TryFrom<$source> for $target {
            type Error = TryFromIntError;

            /// Try to create the target number type from a source
            /// number type. This returns an error if the source value
            /// is outside of the range of the target type.
            #[inline]
            fn try_from(u: $source) -> Result<Self, Self::Error> {
                if u > (Self::max_value() as $source) {
                    Err(TryFromIntError(()))
                } else {
                    Ok(u as Self)
                }
            }
        }
    )*}
}

// all other cases
macro_rules! try_from_both_bounded {
    ($source:ty, $($target:ty),*) => {$(
        #[stable(feature = "try_from", since = "1.34.0")]
        impl TryFrom<$source> for $target {
            type Error = TryFromIntError;

            /// Try to create the target number type from a source
            /// number type. This returns an error if the source value
            /// is outside of the range of the target type.
            #[inline]
            fn try_from(u: $source) -> Result<Self, Self::Error> {
                let min = Self::min_value() as $source;
                let max = Self::max_value() as $source;
                if u < min || u > max {
                    Err(TryFromIntError(()))
                } else {
                    Ok(u as Self)
                }
            }
        }
    )*}
}

macro_rules! rev {
    ($mac:ident, $source:ty, $($target:ty),*) => {$(
        $mac!($target, $source);
    )*}
}

// intra-sign conversions
try_from_upper_bounded!(u16, u8);
try_from_upper_bounded!(u32, u16, u8);
try_from_upper_bounded!(u64, u32, u16, u8);
try_from_upper_bounded!(u128, u64, u32, u16, u8);

try_from_both_bounded!(i16, i8);
try_from_both_bounded!(i32, i16, i8);
try_from_both_bounded!(i64, i32, i16, i8);
try_from_both_bounded!(i128, i64, i32, i16, i8);

// unsigned-to-signed
try_from_upper_bounded!(u8, i8);
try_from_upper_bounded!(u16, i8, i16);
try_from_upper_bounded!(u32, i8, i16, i32);
try_from_upper_bounded!(u64, i8, i16, i32, i64);
try_from_upper_bounded!(u128, i8, i16, i32, i64, i128);

// signed-to-unsigned
try_from_lower_bounded!(i8, u8, u16, u32, u64, u128);
try_from_lower_bounded!(i16, u16, u32, u64, u128);
try_from_lower_bounded!(i32, u32, u64, u128);
try_from_lower_bounded!(i64, u64, u128);
try_from_lower_bounded!(i128, u128);
try_from_both_bounded!(i16, u8);
try_from_both_bounded!(i32, u16, u8);
try_from_both_bounded!(i64, u32, u16, u8);
try_from_both_bounded!(i128, u64, u32, u16, u8);

// usize/isize
try_from_upper_bounded!(usize, isize);
try_from_lower_bounded!(isize, usize);

#[cfg(target_pointer_width = "16")]
mod ptr_try_from_impls {
    use super::TryFromIntError;
    use crate::convert::TryFrom;

    try_from_upper_bounded!(usize, u8);
    try_from_unbounded!(usize, u16, u32, u64, u128);
    try_from_upper_bounded!(usize, i8, i16);
    try_from_unbounded!(usize, i32, i64, i128);

    try_from_both_bounded!(isize, u8);
    try_from_lower_bounded!(isize, u16, u32, u64, u128);
    try_from_both_bounded!(isize, i8);
    try_from_unbounded!(isize, i16, i32, i64, i128);

    rev!(try_from_upper_bounded, usize, u32, u64, u128);
    rev!(try_from_lower_bounded, usize, i8, i16);
    rev!(try_from_both_bounded, usize, i32, i64, i128);

    rev!(try_from_upper_bounded, isize, u16, u32, u64, u128);
    rev!(try_from_both_bounded, isize, i32, i64, i128);
}

#[cfg(target_pointer_width = "32")]
mod ptr_try_from_impls {
    use super::TryFromIntError;
    use crate::convert::TryFrom;

    try_from_upper_bounded!(usize, u8, u16);
    try_from_unbounded!(usize, u32, u64, u128);
    try_from_upper_bounded!(usize, i8, i16, i32);
    try_from_unbounded!(usize, i64, i128);

    try_from_both_bounded!(isize, u8, u16);
    try_from_lower_bounded!(isize, u32, u64, u128);
    try_from_both_bounded!(isize, i8, i16);
    try_from_unbounded!(isize, i32, i64, i128);

    rev!(try_from_unbounded, usize, u32);
    rev!(try_from_upper_bounded, usize, u64, u128);
    rev!(try_from_lower_bounded, usize, i8, i16, i32);
    rev!(try_from_both_bounded, usize, i64, i128);

    rev!(try_from_unbounded, isize, u16);
    rev!(try_from_upper_bounded, isize, u32, u64, u128);
    rev!(try_from_unbounded, isize, i32);
    rev!(try_from_both_bounded, isize, i64, i128);
}

#[cfg(target_pointer_width = "64")]
mod ptr_try_from_impls {
    use super::TryFromIntError;
    use crate::convert::TryFrom;

    try_from_upper_bounded!(usize, u8, u16, u32);
    try_from_unbounded!(usize, u64, u128);
    try_from_upper_bounded!(usize, i8, i16, i32, i64);
    try_from_unbounded!(usize, i128);

    try_from_both_bounded!(isize, u8, u16, u32);
    try_from_lower_bounded!(isize, u64, u128);
    try_from_both_bounded!(isize, i8, i16, i32);
    try_from_unbounded!(isize, i64, i128);

    rev!(try_from_unbounded, usize, u32, u64);
    rev!(try_from_upper_bounded, usize, u128);
    rev!(try_from_lower_bounded, usize, i8, i16, i32, i64);
    rev!(try_from_both_bounded, usize, i128);

    rev!(try_from_unbounded, isize, u16, u32);
    rev!(try_from_upper_bounded, isize, u64, u128);
    rev!(try_from_unbounded, isize, i32, i64);
    rev!(try_from_both_bounded, isize, i128);
}

// Conversion traits for non-zero integer types
use crate::num::NonZeroI128;
use crate::num::NonZeroI16;
use crate::num::NonZeroI32;
use crate::num::NonZeroI64;
use crate::num::NonZeroI8;
use crate::num::NonZeroIsize;
use crate::num::NonZeroU128;
use crate::num::NonZeroU16;
use crate::num::NonZeroU32;
use crate::num::NonZeroU64;
use crate::num::NonZeroU8;
use crate::num::NonZeroUsize;

macro_rules! nzint_impl_from {
    ($Small: ty, $Large: ty, #[$attr:meta], $doc: expr) => {
        #[$attr]
        #[doc = $doc]
        impl From<$Small> for $Large {
            #[inline]
            fn from(small: $Small) -> Self {
                // SAFETY: input type guarantees the value is non-zero
                unsafe {
                    Self::new_unchecked(small.get().into())
                }
            }
        }
    };
    ($Small: ty, $Large: ty, #[$attr:meta]) => {
        nzint_impl_from!($Small,
                   $Large,
                   #[$attr],
                   concat!("Converts `",
                           stringify!($Small),
                           "` to `",
                           stringify!($Large),
                           "` losslessly."));
    }
}

// Non-zero Unsigned -> Non-zero Unsigned
nzint_impl_from! { NonZeroU8, NonZeroU16, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU8, NonZeroU32, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU8, NonZeroU64, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU8, NonZeroU128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU8, NonZeroUsize, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU16, NonZeroU32, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU16, NonZeroU64, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU16, NonZeroU128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU16, NonZeroUsize, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU32, NonZeroU64, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU32, NonZeroU128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU64, NonZeroU128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }

// Non-zero Signed -> Non-zero Signed
nzint_impl_from! { NonZeroI8, NonZeroI16, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI8, NonZeroI32, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI8, NonZeroI64, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI8, NonZeroI128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI8, NonZeroIsize, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI16, NonZeroI32, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI16, NonZeroI64, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI16, NonZeroI128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI16, NonZeroIsize, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI32, NonZeroI64, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI32, NonZeroI128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroI64, NonZeroI128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }

// NonZero UnSigned -> Non-zero Signed
nzint_impl_from! { NonZeroU8, NonZeroI16, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU8, NonZeroI32, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU8, NonZeroI64, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU8, NonZeroI128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU8, NonZeroIsize, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU16, NonZeroI32, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU16, NonZeroI64, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU16, NonZeroI128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU32, NonZeroI64, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU32, NonZeroI128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_from! { NonZeroU64, NonZeroI128, #[stable(feature = "nz_int_conv", since = "1.41.0")] }

macro_rules! nzint_impl_try_from {
    (@PER_ITEM  $NonZeroType: ident; $SameButZeroType: ident; $PossiblyZeroType: ty; #[$attr:meta]; $doc: expr) => {
                #[$attr]
                #[doc = $doc]
                impl TryFrom<$PossiblyZeroType> for $NonZeroType {
                    type Error = TryFromIntError;
                    #[inline]
                    fn try_from(value: $PossiblyZeroType) -> Result<Self,Self::Error> {
                        <$SameButZeroType as TryFrom<$PossiblyZeroType>>::try_from(value)
                            .ok()
                            .into_iter()
                            .flat_map($NonZeroType::new)
                            .next()
                            .ok_or_else(|| TryFromIntError(()))
                    }
                }
    };
    ($NonZeroType: ident, $SameButZeroType: ident => {$($PossiblyZeroType: ty),*} #[$attr:meta]) => {
            $(nzint_impl_try_from!{@PER_ITEM $NonZeroType; $SameButZeroType; $PossiblyZeroType; #[$attr]; concat!("Converts `",stringify!($PossiblyZeroType),"` to `",stringify!($NonZeroType),"` if possible.")})*
    };
}

nzint_impl_try_from! { NonZeroU8, u8 => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroI8, i8 => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroU16, u16 => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroI16, i16 => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroU32, u32 => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroI32, i32 => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroU64, u64 => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroI64, i64 => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroU128, u128 => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroI128, i128 => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroUsize, usize => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
nzint_impl_try_from! { NonZeroIsize, isize => { i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, i128, u128 } #[stable(feature = "nz_int_conv", since = "1.41.0")] }
