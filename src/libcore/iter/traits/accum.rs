use crate::iter;
use crate::num::Wrapping;
use crate::ops::{Add, Mul};

/// Trait to represent types that can be created by summing up an iterator.
///
/// This trait is used to implement the [`sum`] method on iterators. Types which
/// implement the trait can be generated by the [`sum`] method. Like
/// [`FromIterator`] this trait should rarely be called directly and instead
/// interacted with through [`Iterator::sum`].
///
/// [`sum`]: ../../std/iter/trait.Sum.html#tymethod.sum
/// [`FromIterator`]: ../../std/iter/trait.FromIterator.html
/// [`Iterator::sum`]: ../../std/iter/trait.Iterator.html#method.sum
#[stable(feature = "iter_arith_traits", since = "1.12.0")]
pub trait Sum<A = Self>: Sized {
    /// Method which takes an iterator and generates `Self` from the elements by
    /// "summing up" the items.
    #[stable(feature = "iter_arith_traits", since = "1.12.0")]
    fn sum<I: Iterator<Item = A>>(iter: I) -> Self;
}

/// Trait to represent types that can be created by multiplying elements of an
/// iterator.
///
/// This trait is used to implement the [`product`] method on iterators. Types
/// which implement the trait can be generated by the [`product`] method. Like
/// [`FromIterator`] this trait should rarely be called directly and instead
/// interacted with through [`Iterator::product`].
///
/// [`product`]: ../../std/iter/trait.Product.html#tymethod.product
/// [`FromIterator`]: ../../std/iter/trait.FromIterator.html
/// [`Iterator::product`]: ../../std/iter/trait.Iterator.html#method.product
#[stable(feature = "iter_arith_traits", since = "1.12.0")]
pub trait Product<A = Self>: Sized {
    /// Method which takes an iterator and generates `Self` from the elements by
    /// multiplying the items.
    #[stable(feature = "iter_arith_traits", since = "1.12.0")]
    fn product<I: Iterator<Item = A>>(iter: I) -> Self;
}

// N.B., explicitly use Add and Mul here to inherit overflow checks
macro_rules! integer_sum_product {
    (@impls $zero:expr, $one:expr, #[$attr:meta], $($a:ty)*) => ($(
        #[$attr]
        impl Sum for $a {
            fn sum<I: Iterator<Item=Self>>(iter: I) -> Self {
                iter.fold($zero, Add::add)
            }
        }

        #[$attr]
        impl Product for $a {
            fn product<I: Iterator<Item=Self>>(iter: I) -> Self {
                iter.fold($one, Mul::mul)
            }
        }

        #[$attr]
        impl<'a> Sum<&'a $a> for $a {
            fn sum<I: Iterator<Item=&'a Self>>(iter: I) -> Self {
                iter.fold($zero, Add::add)
            }
        }

        #[$attr]
        impl<'a> Product<&'a $a> for $a {
            fn product<I: Iterator<Item=&'a Self>>(iter: I) -> Self {
                iter.fold($one, Mul::mul)
            }
        }
    )*);
    ($($a:ty)*) => (
        integer_sum_product!(@impls 0, 1,
                #[stable(feature = "iter_arith_traits", since = "1.12.0")],
                $($a)*);
        integer_sum_product!(@impls Wrapping(0), Wrapping(1),
                #[stable(feature = "wrapping_iter_arith", since = "1.14.0")],
                $(Wrapping<$a>)*);
    );
}

macro_rules! float_sum_product {
    ($($a:ident)*) => ($(
        #[stable(feature = "iter_arith_traits", since = "1.12.0")]
        impl Sum for $a {
            fn sum<I: Iterator<Item=Self>>(iter: I) -> Self {
                iter.fold(0.0, Add::add)
            }
        }

        #[stable(feature = "iter_arith_traits", since = "1.12.0")]
        impl Product for $a {
            fn product<I: Iterator<Item=Self>>(iter: I) -> Self {
                iter.fold(1.0, Mul::mul)
            }
        }

        #[stable(feature = "iter_arith_traits", since = "1.12.0")]
        impl<'a> Sum<&'a $a> for $a {
            fn sum<I: Iterator<Item=&'a Self>>(iter: I) -> Self {
                iter.fold(0.0, Add::add)
            }
        }

        #[stable(feature = "iter_arith_traits", since = "1.12.0")]
        impl<'a> Product<&'a $a> for $a {
            fn product<I: Iterator<Item=&'a Self>>(iter: I) -> Self {
                iter.fold(1.0, Mul::mul)
            }
        }
    )*)
}

integer_sum_product! { i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize }
float_sum_product! { f32 f64 }

#[stable(feature = "iter_arith_traits_result", since = "1.16.0")]
impl<T, U, E> Sum<Result<U, E>> for Result<T, E>
where
    T: Sum<U>,
{
    /// Takes each element in the `Iterator`: if it is an `Err`, no further
    /// elements are taken, and the `Err` is returned. Should no `Err` occur,
    /// the sum of all elements is returned.
    ///
    /// # Examples
    ///
    /// This sums up every integer in a vector, rejecting the sum if a negative
    /// element is encountered:
    ///
    /// ```
    /// let v = vec![1, 2];
    /// let res: Result<i32, &'static str> = v.iter().map(|&x: &i32|
    ///     if x < 0 { Err("Negative element found") }
    ///     else { Ok(x) }
    /// ).sum();
    /// assert_eq!(res, Ok(3));
    /// ```
    fn sum<I>(iter: I) -> Result<T, E>
    where
        I: Iterator<Item = Result<U, E>>,
    {
        iter::process_results(iter, |i| i.sum())
    }
}

#[stable(feature = "iter_arith_traits_result", since = "1.16.0")]
impl<T, U, E> Product<Result<U, E>> for Result<T, E>
where
    T: Product<U>,
{
    /// Takes each element in the `Iterator`: if it is an `Err`, no further
    /// elements are taken, and the `Err` is returned. Should no `Err` occur,
    /// the product of all elements is returned.
    fn product<I>(iter: I) -> Result<T, E>
    where
        I: Iterator<Item = Result<U, E>>,
    {
        iter::process_results(iter, |i| i.product())
    }
}

#[stable(feature = "iter_arith_traits_option", since = "1.37.0")]
impl<T, U> Sum<Option<U>> for Option<T>
where
    T: Sum<U>,
{
    /// Takes each element in the `Iterator`: if it is a `None`, no further
    /// elements are taken, and the `None` is returned. Should no `None` occur,
    /// the sum of all elements is returned.
    ///
    /// # Examples
    ///
    /// This sums up the position of the character 'a' in a vector of strings,
    /// if a word did not have the character 'a' the operation returns `None`:
    ///
    /// ```
    /// let words = vec!["have", "a", "great", "day"];
    /// let total: Option<usize> = words.iter().map(|w| w.find('a')).sum();
    /// assert_eq!(total, Some(5));
    /// ```
    fn sum<I>(iter: I) -> Option<T>
    where
        I: Iterator<Item = Option<U>>,
    {
        iter.map(|x| x.ok_or(())).sum::<Result<_, _>>().ok()
    }
}

#[stable(feature = "iter_arith_traits_option", since = "1.37.0")]
impl<T, U> Product<Option<U>> for Option<T>
where
    T: Product<U>,
{
    /// Takes each element in the `Iterator`: if it is a `None`, no further
    /// elements are taken, and the `None` is returned. Should no `None` occur,
    /// the product of all elements is returned.
    fn product<I>(iter: I) -> Option<T>
    where
        I: Iterator<Item = Option<U>>,
    {
        iter.map(|x| x.ok_or(())).product::<Result<_, _>>().ok()
    }
}
