/// The unary logical negation operator `!`.
///
/// # Examples
///
/// An implementation of `Not` for `Answer`, which enables the use of `!` to
/// invert its value.
///
/// ```
/// use std::ops::Not;
///
/// #[derive(Debug, PartialEq)]
/// enum Answer {
///     Yes,
///     No,
/// }
///
/// impl Not for Answer {
///     type Output = Answer;
///
///     fn not(self) -> Answer {
///         match self {
///             Answer::Yes => Answer::No,
///             Answer::No => Answer::Yes
///         }
///     }
/// }
///
/// assert_eq!(!Answer::Yes, Answer::No);
/// assert_eq!(!Answer::No, Answer::Yes);
/// ```
#[lang = "not"]
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Not {
    /// The resulting type after applying the `!` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// The method for the unary `!` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    fn not(self) -> Self::Output;
}

macro_rules! not_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        impl Not for $t {
            type Output = $t;

            #[inline]
            fn not(self) -> $t { !self }
        }

        forward_ref_unop! { impl Not, not for $t }
    )*)
}

not_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// The bitwise AND operator `&`.
///
/// # Examples
///
/// In this example, the `&` operator is lifted to a trivial `Scalar` type.
///
/// ```
/// use std::ops::BitAnd;
///
/// #[derive(Debug, PartialEq)]
/// struct Scalar(bool);
///
/// impl BitAnd for Scalar {
///     type Output = Self;
///
///     // rhs is the "right-hand side" of the expression `a & b`
///     fn bitand(self, rhs: Self) -> Self {
///         Scalar(self.0 & rhs.0)
///     }
/// }
///
/// fn main() {
///     assert_eq!(Scalar(true) & Scalar(true), Scalar(true));
///     assert_eq!(Scalar(true) & Scalar(false), Scalar(false));
///     assert_eq!(Scalar(false) & Scalar(true), Scalar(false));
///     assert_eq!(Scalar(false) & Scalar(false), Scalar(false));
/// }
/// ```
///
/// In this example, the `BitAnd` trait is implemented for a `BooleanVector`
/// struct.
///
/// ```
/// use std::ops::BitAnd;
///
/// #[derive(Debug, PartialEq)]
/// struct BooleanVector(Vec<bool>);
///
/// impl BitAnd for BooleanVector {
///     type Output = Self;
///
///     fn bitand(self, BooleanVector(rhs): Self) -> Self {
///         let BooleanVector(lhs) = self;
///         assert_eq!(lhs.len(), rhs.len());
///         BooleanVector(lhs.iter().zip(rhs.iter()).map(|(x, y)| *x && *y).collect())
///     }
/// }
///
/// fn main() {
///     let bv1 = BooleanVector(vec![true, true, false, false]);
///     let bv2 = BooleanVector(vec![true, false, true, false]);
///     let expected = BooleanVector(vec![true, false, false, false]);
///     assert_eq!(bv1 & bv2, expected);
/// }
/// ```
#[lang = "bitand"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_on_unimplemented = "no implementation for `{Self} & {RHS}`"]
pub trait BitAnd<RHS=Self> {
    /// The resulting type after applying the `&` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// The method for the `&` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    fn bitand(self, rhs: RHS) -> Self::Output;
}

macro_rules! bitand_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        impl BitAnd for $t {
            type Output = $t;

            #[inline]
            fn bitand(self, rhs: $t) -> $t { self & rhs }
        }

        forward_ref_binop! { impl BitAnd, bitand for $t, $t }
    )*)
}

bitand_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// The bitwise OR operator `|`.
///
/// # Examples
///
/// In this example, the `|` operator is lifted to a trivial `Scalar` type.
///
/// ```
/// use std::ops::BitOr;
///
/// #[derive(Debug, PartialEq)]
/// struct Scalar(bool);
///
/// impl BitOr for Scalar {
///     type Output = Self;
///
///     // rhs is the "right-hand side" of the expression `a | b`
///     fn bitor(self, rhs: Self) -> Self {
///         Scalar(self.0 | rhs.0)
///     }
/// }
///
/// fn main() {
///     assert_eq!(Scalar(true) | Scalar(true), Scalar(true));
///     assert_eq!(Scalar(true) | Scalar(false), Scalar(true));
///     assert_eq!(Scalar(false) | Scalar(true), Scalar(true));
///     assert_eq!(Scalar(false) | Scalar(false), Scalar(false));
/// }
/// ```
///
/// In this example, the `BitOr` trait is implemented for a `BooleanVector`
/// struct.
///
/// ```
/// use std::ops::BitOr;
///
/// #[derive(Debug, PartialEq)]
/// struct BooleanVector(Vec<bool>);
///
/// impl BitOr for BooleanVector {
///     type Output = Self;
///
///     fn bitor(self, BooleanVector(rhs): Self) -> Self {
///         let BooleanVector(lhs) = self;
///         assert_eq!(lhs.len(), rhs.len());
///         BooleanVector(lhs.iter().zip(rhs.iter()).map(|(x, y)| *x || *y).collect())
///     }
/// }
///
/// fn main() {
///     let bv1 = BooleanVector(vec![true, true, false, false]);
///     let bv2 = BooleanVector(vec![true, false, true, false]);
///     let expected = BooleanVector(vec![true, true, true, false]);
///     assert_eq!(bv1 | bv2, expected);
/// }
/// ```
#[lang = "bitor"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_on_unimplemented = "no implementation for `{Self} | {RHS}`"]
pub trait BitOr<RHS=Self> {
    /// The resulting type after applying the `|` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// The method for the `|` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    fn bitor(self, rhs: RHS) -> Self::Output;
}

macro_rules! bitor_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        impl BitOr for $t {
            type Output = $t;

            #[inline]
            fn bitor(self, rhs: $t) -> $t { self | rhs }
        }

        forward_ref_binop! { impl BitOr, bitor for $t, $t }
    )*)
}

bitor_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// The bitwise XOR operator `^`.
///
/// # Examples
///
/// In this example, the `^` operator is lifted to a trivial `Scalar` type.
///
/// ```
/// use std::ops::BitXor;
///
/// #[derive(Debug, PartialEq)]
/// struct Scalar(bool);
///
/// impl BitXor for Scalar {
///     type Output = Self;
///
///     // rhs is the "right-hand side" of the expression `a ^ b`
///     fn bitxor(self, rhs: Self) -> Self {
///         Scalar(self.0 ^ rhs.0)
///     }
/// }
///
/// fn main() {
///     assert_eq!(Scalar(true) ^ Scalar(true), Scalar(false));
///     assert_eq!(Scalar(true) ^ Scalar(false), Scalar(true));
///     assert_eq!(Scalar(false) ^ Scalar(true), Scalar(true));
///     assert_eq!(Scalar(false) ^ Scalar(false), Scalar(false));
/// }
/// ```
///
/// In this example, the `BitXor` trait is implemented for a `BooleanVector`
/// struct.
///
/// ```
/// use std::ops::BitXor;
///
/// #[derive(Debug, PartialEq)]
/// struct BooleanVector(Vec<bool>);
///
/// impl BitXor for BooleanVector {
///     type Output = Self;
///
///     fn bitxor(self, BooleanVector(rhs): Self) -> Self {
///         let BooleanVector(lhs) = self;
///         assert_eq!(lhs.len(), rhs.len());
///         BooleanVector(lhs.iter()
///                          .zip(rhs.iter())
///                          .map(|(x, y)| (*x || *y) && !(*x && *y))
///                          .collect())
///     }
/// }
///
/// fn main() {
///     let bv1 = BooleanVector(vec![true, true, false, false]);
///     let bv2 = BooleanVector(vec![true, false, true, false]);
///     let expected = BooleanVector(vec![false, true, true, false]);
///     assert_eq!(bv1 ^ bv2, expected);
/// }
/// ```
#[lang = "bitxor"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_on_unimplemented = "no implementation for `{Self} ^ {RHS}`"]
pub trait BitXor<RHS=Self> {
    /// The resulting type after applying the `^` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// The method for the `^` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    fn bitxor(self, rhs: RHS) -> Self::Output;
}

macro_rules! bitxor_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        impl BitXor for $t {
            type Output = $t;

            #[inline]
            fn bitxor(self, other: $t) -> $t { self ^ other }
        }

        forward_ref_binop! { impl BitXor, bitxor for $t, $t }
    )*)
}

bitxor_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// The left shift operator `<<`.
///
/// # Examples
///
/// An implementation of `Shl` that lifts the `<<` operation on integers to a
/// `Scalar` struct.
///
/// ```
/// use std::ops::Shl;
///
/// #[derive(PartialEq, Debug)]
/// struct Scalar(usize);
///
/// impl Shl<Scalar> for Scalar {
///     type Output = Self;
///
///     fn shl(self, Scalar(rhs): Self) -> Scalar {
///         let Scalar(lhs) = self;
///         Scalar(lhs << rhs)
///     }
/// }
/// fn main() {
///     assert_eq!(Scalar(4) << Scalar(2), Scalar(16));
/// }
/// ```
///
/// An implementation of `Shl` that spins a vector leftward by a given amount.
///
/// ```
/// use std::ops::Shl;
///
/// #[derive(PartialEq, Debug)]
/// struct SpinVector<T: Clone> {
///     vec: Vec<T>,
/// }
///
/// impl<T: Clone> Shl<usize> for SpinVector<T> {
///     type Output = Self;
///
///     fn shl(self, rhs: usize) -> SpinVector<T> {
///         // rotate the vector by `rhs` places
///         let (a, b) = self.vec.split_at(rhs);
///         let mut spun_vector: Vec<T> = vec![];
///         spun_vector.extend_from_slice(b);
///         spun_vector.extend_from_slice(a);
///         SpinVector { vec: spun_vector }
///     }
/// }
///
/// fn main() {
///     assert_eq!(SpinVector { vec: vec![0, 1, 2, 3, 4] } << 2,
///                SpinVector { vec: vec![2, 3, 4, 0, 1] });
/// }
/// ```
#[lang = "shl"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_on_unimplemented = "no implementation for `{Self} << {RHS}`"]
pub trait Shl<RHS> {
    /// The resulting type after applying the `<<` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// The method for the `<<` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    fn shl(self, rhs: RHS) -> Self::Output;
}

macro_rules! shl_impl {
    ($t:ty, $f:ty) => (
        #[stable(feature = "rust1", since = "1.0.0")]
        impl Shl<$f> for $t {
            type Output = $t;

            #[inline]
            #[rustc_inherit_overflow_checks]
            fn shl(self, other: $f) -> $t {
                self << other
            }
        }

        forward_ref_binop! { impl Shl, shl for $t, $f }
    )
}

macro_rules! shl_impl_all {
    ($($t:ty)*) => ($(
        shl_impl! { $t, u8 }
        shl_impl! { $t, u16 }
        shl_impl! { $t, u32 }
        shl_impl! { $t, u64 }
        shl_impl! { $t, u128 }
        shl_impl! { $t, usize }

        shl_impl! { $t, i8 }
        shl_impl! { $t, i16 }
        shl_impl! { $t, i32 }
        shl_impl! { $t, i64 }
        shl_impl! { $t, i128 }
        shl_impl! { $t, isize }
    )*)
}

shl_impl_all! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 isize i128 }

/// The right shift operator `>>`.
///
/// # Examples
///
/// An implementation of `Shr` that lifts the `>>` operation on integers to a
/// `Scalar` struct.
///
/// ```
/// use std::ops::Shr;
///
/// #[derive(PartialEq, Debug)]
/// struct Scalar(usize);
///
/// impl Shr<Scalar> for Scalar {
///     type Output = Self;
///
///     fn shr(self, Scalar(rhs): Self) -> Scalar {
///         let Scalar(lhs) = self;
///         Scalar(lhs >> rhs)
///     }
/// }
/// fn main() {
///     assert_eq!(Scalar(16) >> Scalar(2), Scalar(4));
/// }
/// ```
///
/// An implementation of `Shr` that spins a vector rightward by a given amount.
///
/// ```
/// use std::ops::Shr;
///
/// #[derive(PartialEq, Debug)]
/// struct SpinVector<T: Clone> {
///     vec: Vec<T>,
/// }
///
/// impl<T: Clone> Shr<usize> for SpinVector<T> {
///     type Output = Self;
///
///     fn shr(self, rhs: usize) -> SpinVector<T> {
///         // rotate the vector by `rhs` places
///         let (a, b) = self.vec.split_at(self.vec.len() - rhs);
///         let mut spun_vector: Vec<T> = vec![];
///         spun_vector.extend_from_slice(b);
///         spun_vector.extend_from_slice(a);
///         SpinVector { vec: spun_vector }
///     }
/// }
///
/// fn main() {
///     assert_eq!(SpinVector { vec: vec![0, 1, 2, 3, 4] } >> 2,
///                SpinVector { vec: vec![3, 4, 0, 1, 2] });
/// }
/// ```
#[lang = "shr"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_on_unimplemented = "no implementation for `{Self} >> {RHS}`"]
pub trait Shr<RHS> {
    /// The resulting type after applying the `>>` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output;

    /// The method for the `>>` operator
    #[stable(feature = "rust1", since = "1.0.0")]
    fn shr(self, rhs: RHS) -> Self::Output;
}

macro_rules! shr_impl {
    ($t:ty, $f:ty) => (
        #[stable(feature = "rust1", since = "1.0.0")]
        impl Shr<$f> for $t {
            type Output = $t;

            #[inline]
            #[rustc_inherit_overflow_checks]
            fn shr(self, other: $f) -> $t {
                self >> other
            }
        }

        forward_ref_binop! { impl Shr, shr for $t, $f }
    )
}

macro_rules! shr_impl_all {
    ($($t:ty)*) => ($(
        shr_impl! { $t, u8 }
        shr_impl! { $t, u16 }
        shr_impl! { $t, u32 }
        shr_impl! { $t, u64 }
        shr_impl! { $t, u128 }
        shr_impl! { $t, usize }

        shr_impl! { $t, i8 }
        shr_impl! { $t, i16 }
        shr_impl! { $t, i32 }
        shr_impl! { $t, i64 }
        shr_impl! { $t, i128 }
        shr_impl! { $t, isize }
    )*)
}

shr_impl_all! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }

/// The bitwise AND assignment operator `&=`.
///
/// # Examples
///
/// In this example, the `&=` operator is lifted to a trivial `Scalar` type.
///
/// ```
/// use std::ops::BitAndAssign;
///
/// #[derive(Debug, PartialEq)]
/// struct Scalar(bool);
///
/// impl BitAndAssign for Scalar {
///     // rhs is the "right-hand side" of the expression `a &= b`
///     fn bitand_assign(&mut self, rhs: Self) {
///         *self = Scalar(self.0 & rhs.0)
///     }
/// }
///
/// fn main() {
///     let mut scalar = Scalar(true);
///     scalar &= Scalar(true);
///     assert_eq!(scalar, Scalar(true));
///
///     let mut scalar = Scalar(true);
///     scalar &= Scalar(false);
///     assert_eq!(scalar, Scalar(false));
///
///     let mut scalar = Scalar(false);
///     scalar &= Scalar(true);
///     assert_eq!(scalar, Scalar(false));
///
///     let mut scalar = Scalar(false);
///     scalar &= Scalar(false);
///     assert_eq!(scalar, Scalar(false));
/// }
/// ```
///
/// In this example, the `BitAndAssign` trait is implemented for a
/// `BooleanVector` struct.
///
/// ```
/// use std::ops::BitAndAssign;
///
/// #[derive(Debug, PartialEq)]
/// struct BooleanVector(Vec<bool>);
///
/// impl BitAndAssign for BooleanVector {
///     // rhs is the "right-hand side" of the expression `a &= b`
///     fn bitand_assign(&mut self, rhs: Self) {
///         assert_eq!(self.0.len(), rhs.0.len());
///         *self = BooleanVector(self.0
///                                   .iter()
///                                   .zip(rhs.0.iter())
///                                   .map(|(x, y)| *x && *y)
///                                   .collect());
///     }
/// }
///
/// fn main() {
///     let mut bv = BooleanVector(vec![true, true, false, false]);
///     bv &= BooleanVector(vec![true, false, true, false]);
///     let expected = BooleanVector(vec![true, false, false, false]);
///     assert_eq!(bv, expected);
/// }
/// ```
#[lang = "bitand_assign"]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_on_unimplemented = "no implementation for `{Self} &= {Rhs}`"]
pub trait BitAndAssign<Rhs=Self> {
    /// The method for the `&=` operator
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn bitand_assign(&mut self, rhs: Rhs);
}

macro_rules! bitand_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        impl BitAndAssign for $t {
            #[inline]
            fn bitand_assign(&mut self, other: $t) { *self &= other }
        }
    )+)
}

bitand_assign_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// The bitwise OR assignment operator `|=`.
///
/// # Examples
///
/// A trivial implementation of `BitOrAssign`. When `Foo |= Foo` happens, it ends up
/// calling `bitor_assign`, and therefore, `main` prints `Bitwise Or-ing!`.
///
/// ```
/// use std::ops::BitOrAssign;
///
/// struct Foo;
///
/// impl BitOrAssign for Foo {
///     fn bitor_assign(&mut self, _rhs: Foo) {
///         println!("Bitwise Or-ing!");
///     }
/// }
///
/// # #[allow(unused_assignments)]
/// fn main() {
///     let mut foo = Foo;
///     foo |= Foo;
/// }
/// ```
#[lang = "bitor_assign"]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_on_unimplemented = "no implementation for `{Self} |= {Rhs}`"]
pub trait BitOrAssign<Rhs=Self> {
    /// The method for the `|=` operator
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn bitor_assign(&mut self, rhs: Rhs);
}

macro_rules! bitor_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        impl BitOrAssign for $t {
            #[inline]
            fn bitor_assign(&mut self, other: $t) { *self |= other }
        }
    )+)
}

bitor_assign_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// The bitwise XOR assignment operator `^=`.
///
/// # Examples
///
/// A trivial implementation of `BitXorAssign`. When `Foo ^= Foo` happens, it ends up
/// calling `bitxor_assign`, and therefore, `main` prints `Bitwise Xor-ing!`.
///
/// ```
/// use std::ops::BitXorAssign;
///
/// struct Foo;
///
/// impl BitXorAssign for Foo {
///     fn bitxor_assign(&mut self, _rhs: Foo) {
///         println!("Bitwise Xor-ing!");
///     }
/// }
///
/// # #[allow(unused_assignments)]
/// fn main() {
///     let mut foo = Foo;
///     foo ^= Foo;
/// }
/// ```
#[lang = "bitxor_assign"]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_on_unimplemented = "no implementation for `{Self} ^= {Rhs}`"]
pub trait BitXorAssign<Rhs=Self> {
    /// The method for the `^=` operator
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn bitxor_assign(&mut self, rhs: Rhs);
}

macro_rules! bitxor_assign_impl {
    ($($t:ty)+) => ($(
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        impl BitXorAssign for $t {
            #[inline]
            fn bitxor_assign(&mut self, other: $t) { *self ^= other }
        }
    )+)
}

bitxor_assign_impl! { bool usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

/// The left shift assignment operator `<<=`.
///
/// # Examples
///
/// A trivial implementation of `ShlAssign`. When `Foo <<= Foo` happens, it ends up
/// calling `shl_assign`, and therefore, `main` prints `Shifting left!`.
///
/// ```
/// use std::ops::ShlAssign;
///
/// struct Foo;
///
/// impl ShlAssign<Foo> for Foo {
///     fn shl_assign(&mut self, _rhs: Foo) {
///         println!("Shifting left!");
///     }
/// }
///
/// # #[allow(unused_assignments)]
/// fn main() {
///     let mut foo = Foo;
///     foo <<= Foo;
/// }
/// ```
#[lang = "shl_assign"]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_on_unimplemented = "no implementation for `{Self} <<= {Rhs}`"]
pub trait ShlAssign<Rhs> {
    /// The method for the `<<=` operator
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn shl_assign(&mut self, rhs: Rhs);
}

macro_rules! shl_assign_impl {
    ($t:ty, $f:ty) => (
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        impl ShlAssign<$f> for $t {
            #[inline]
            #[rustc_inherit_overflow_checks]
            fn shl_assign(&mut self, other: $f) {
                *self <<= other
            }
        }
    )
}

macro_rules! shl_assign_impl_all {
    ($($t:ty)*) => ($(
        shl_assign_impl! { $t, u8 }
        shl_assign_impl! { $t, u16 }
        shl_assign_impl! { $t, u32 }
        shl_assign_impl! { $t, u64 }
        shl_assign_impl! { $t, u128 }
        shl_assign_impl! { $t, usize }

        shl_assign_impl! { $t, i8 }
        shl_assign_impl! { $t, i16 }
        shl_assign_impl! { $t, i32 }
        shl_assign_impl! { $t, i64 }
        shl_assign_impl! { $t, i128 }
        shl_assign_impl! { $t, isize }
    )*)
}

shl_assign_impl_all! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }

/// The right shift assignment operator `>>=`.
///
/// # Examples
///
/// A trivial implementation of `ShrAssign`. When `Foo >>= Foo` happens, it ends up
/// calling `shr_assign`, and therefore, `main` prints `Shifting right!`.
///
/// ```
/// use std::ops::ShrAssign;
///
/// struct Foo;
///
/// impl ShrAssign<Foo> for Foo {
///     fn shr_assign(&mut self, _rhs: Foo) {
///         println!("Shifting right!");
///     }
/// }
///
/// # #[allow(unused_assignments)]
/// fn main() {
///     let mut foo = Foo;
///     foo >>= Foo;
/// }
/// ```
#[lang = "shr_assign"]
#[stable(feature = "op_assign_traits", since = "1.8.0")]
#[rustc_on_unimplemented = "no implementation for `{Self} >>= {Rhs}`"]
pub trait ShrAssign<Rhs=Self> {
    /// The method for the `>>=` operator
    #[stable(feature = "op_assign_traits", since = "1.8.0")]
    fn shr_assign(&mut self, rhs: Rhs);
}

macro_rules! shr_assign_impl {
    ($t:ty, $f:ty) => (
        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        impl ShrAssign<$f> for $t {
            #[inline]
            #[rustc_inherit_overflow_checks]
            fn shr_assign(&mut self, other: $f) {
                *self >>= other
            }
        }
    )
}

macro_rules! shr_assign_impl_all {
    ($($t:ty)*) => ($(
        shr_assign_impl! { $t, u8 }
        shr_assign_impl! { $t, u16 }
        shr_assign_impl! { $t, u32 }
        shr_assign_impl! { $t, u64 }
        shr_assign_impl! { $t, u128 }
        shr_assign_impl! { $t, usize }

        shr_assign_impl! { $t, i8 }
        shr_assign_impl! { $t, i16 }
        shr_assign_impl! { $t, i32 }
        shr_assign_impl! { $t, i64 }
        shr_assign_impl! { $t, i128 }
        shr_assign_impl! { $t, isize }
    )*)
}

shr_assign_impl_all! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }

