// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![macro_escape]
#![doc(hidden)]

macro_rules! uint_module (($T:ty, $T_SIGNED:ty, $bits:expr) => (

pub static BITS : uint = $bits;
pub static BYTES : uint = ($bits / 8);

pub static MIN: $T = 0 as $T;
pub static MAX: $T = 0 as $T - 1 as $T;

#[cfg(test)]
mod tests {
    use prelude::*;
    use super::*;

    use num;
    use num::CheckedDiv;

    #[test]
    fn test_overflows() {
        assert!(MAX > 0);
        assert!(MIN <= 0);
        assert!(MIN + MAX + 1 == 0);
    }

    #[test]
    fn test_num() {
        num::test_num(10 as $T, 2 as $T);
    }

    #[test]
    fn test_bitwise_operators() {
        assert!(0b1110 as $T == (0b1100 as $T).bitor(&(0b1010 as $T)));
        assert!(0b1000 as $T == (0b1100 as $T).bitand(&(0b1010 as $T)));
        assert!(0b0110 as $T == (0b1100 as $T).bitxor(&(0b1010 as $T)));
        assert!(0b1110 as $T == (0b0111 as $T).shl(&(1 as $T)));
        assert!(0b0111 as $T == (0b1110 as $T).shr(&(1 as $T)));
        assert!(MAX - (0b1011 as $T) == (0b1011 as $T).not());
    }

    static A: $T = 0b0101100;
    static B: $T = 0b0100001;
    static C: $T = 0b1111001;

    static _0: $T = 0;
    static _1: $T = !0;

    #[test]
    fn test_count_ones() {
        assert!(A.count_ones() == 3);
        assert!(B.count_ones() == 2);
        assert!(C.count_ones() == 5);
    }

    #[test]
    fn test_count_zeros() {
        assert!(A.count_zeros() == BITS as $T - 3);
        assert!(B.count_zeros() == BITS as $T - 2);
        assert!(C.count_zeros() == BITS as $T - 5);
    }

    #[test]
    fn test_rotate() {
        assert_eq!(A.rotate_left(6).rotate_right(2).rotate_right(4), A);
        assert_eq!(B.rotate_left(3).rotate_left(2).rotate_right(5), B);
        assert_eq!(C.rotate_left(6).rotate_right(2).rotate_right(4), C);

        // Rotating these should make no difference
        //
        // We test using 124 bits because to ensure that overlong bit shifts do
        // not cause undefined behaviour. See #10183.
        assert_eq!(_0.rotate_left(124), _0);
        assert_eq!(_1.rotate_left(124), _1);
        assert_eq!(_0.rotate_right(124), _0);
        assert_eq!(_1.rotate_right(124), _1);
    }

    #[test]
    fn test_swap_bytes() {
        assert_eq!(A.swap_bytes().swap_bytes(), A);
        assert_eq!(B.swap_bytes().swap_bytes(), B);
        assert_eq!(C.swap_bytes().swap_bytes(), C);

        // Swapping these should make no difference
        assert_eq!(_0.swap_bytes(), _0);
        assert_eq!(_1.swap_bytes(), _1);
    }

    #[test]
    fn test_little_endian() {
        assert_eq!(Int::from_little_endian(A.to_little_endian()), A);
        assert_eq!(Int::from_little_endian(B.to_little_endian()), B);
        assert_eq!(Int::from_little_endian(C.to_little_endian()), C);
        assert_eq!(Int::from_little_endian(_0), _0);
        assert_eq!(Int::from_little_endian(_1), _1);
        assert_eq!(_0.to_little_endian(), _0);
        assert_eq!(_1.to_little_endian(), _1);
    }

    #[test]
    fn test_big_endian() {
        assert_eq!(Int::from_big_endian(A.to_big_endian()), A);
        assert_eq!(Int::from_big_endian(B.to_big_endian()), B);
        assert_eq!(Int::from_big_endian(C.to_big_endian()), C);
        assert_eq!(Int::from_big_endian(_0), _0);
        assert_eq!(Int::from_big_endian(_1), _1);
        assert_eq!(_0.to_big_endian(), _0);
        assert_eq!(_1.to_big_endian(), _1);
    }

    #[test]
    fn test_unsigned_checked_div() {
        assert!(10u.checked_div(&2) == Some(5));
        assert!(5u.checked_div(&0) == None);
    }
}

))
