// revisions: min_tait full_tait
#![feature(min_type_alias_impl_trait)]
#![cfg_attr(full_tait, feature(type_alias_impl_trait))]
//[full_tait]~^ WARN incomplete

trait IterBits {
    type BitsIter: Iterator<Item = u8>;
    fn iter_bits(self, n: u8) -> Self::BitsIter;
}

type IterBitsIter<T, E, I> = impl std::iter::Iterator<Item = I>;

impl<T: Copy, E> IterBits for T
where
    T: std::ops::Shr<Output = T>
        + std::ops::BitAnd<T, Output = T>
        + std::convert::From<u8>
        + std::convert::TryInto<u8, Error = E>,
    E: std::fmt::Debug,
{
    type BitsIter = IterBitsIter<T, E, u8>;
    fn iter_bits(self, n: u8) -> Self::BitsIter {
        //~^ ERROR non-defining opaque type use in defining scope
        (0u8..n).rev().map(move |shift| ((self >> T::from(shift)) & T::from(1)).try_into().unwrap())
    }
}

fn main() {}
