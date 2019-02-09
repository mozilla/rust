#![allow(unused_imports)]
#![deny(unused_assignments)]

use std::mem;
use std::ops::{
    AddAssign, BitAndAssign, BitOrAssign, BitXorAssign, DivAssign, Index, MulAssign, RemAssign,
    ShlAssign, ShrAssign, SubAssign,
};

#[derive(Debug, PartialEq)]
struct Int(i32);

struct Slice([i32]);

impl Slice {
    fn new(slice: &mut [i32]) -> &mut Slice {
        unsafe {
            mem::transmute(slice)
        }
    }
}

struct View<'a>(&'a mut [i32]);

fn main() {
    let mut x = Int(1);

    x += Int(2);
    assert_eq!(x, Int(0b11));

    x &= Int(0b01);
    assert_eq!(x, Int(0b01));

    x |= Int(0b10);
    assert_eq!(x, Int(0b11));

    x ^= Int(0b01);
    assert_eq!(x, Int(0b10));

    x /= Int(2);
    assert_eq!(x, Int(1));

    x *= Int(3);
    assert_eq!(x, Int(3));

    x %= Int(2);
    assert_eq!(x, Int(1));

    // overloaded RHS
    x <<= 1u8;
    assert_eq!(x, Int(2));

    x <<= 1u16;
    assert_eq!(x, Int(4));

    x >>= 1u8;
    assert_eq!(x, Int(2));

    x >>= 1u16;
    assert_eq!(x, Int(1));

    x -= Int(1);
    assert_eq!(x, Int(0));

    // Indexed LHS.
    let mut v = vec![Int(1), Int(2)];
    v[0] += Int(2);
    assert_eq!(v[0], Int(3));

    // Unsized RHS.
    let mut array = [0, 1, 2];
    *Slice::new(&mut array) += 1;
    assert_eq!(array[0], 1);
    assert_eq!(array[1], 2);
    assert_eq!(array[2], 3);

    // Sized indirection.
    // Check that this does *not* trigger the `unused_assignments` lint.
    let mut array = [0, 1, 2];
    let mut view = View(&mut array);
    view += 1;
}

impl AddAssign for Int {
    fn add_assign(&mut self, rhs: Int) {
        self.0 += rhs.0;
    }
}

impl BitAndAssign for Int {
    fn bitand_assign(&mut self, rhs: Int) {
        self.0 &= rhs.0;
    }
}

impl BitOrAssign for Int {
    fn bitor_assign(&mut self, rhs: Int) {
        self.0 |= rhs.0;
    }
}

impl BitXorAssign for Int {
    fn bitxor_assign(&mut self, rhs: Int) {
        self.0 ^= rhs.0;
    }
}

impl DivAssign for Int {
    fn div_assign(&mut self, rhs: Int) {
        self.0 /= rhs.0;
    }
}

impl MulAssign for Int {
    fn mul_assign(&mut self, rhs: Int) {
        self.0 *= rhs.0;
    }
}

impl RemAssign for Int {
    fn rem_assign(&mut self, rhs: Int) {
        self.0 %= rhs.0;
    }
}

impl ShlAssign<u8> for Int {
    fn shl_assign(&mut self, rhs: u8) {
        self.0 <<= rhs;
    }
}

impl ShlAssign<u16> for Int {
    fn shl_assign(&mut self, rhs: u16) {
        self.0 <<= rhs;
    }
}

impl ShrAssign<u8> for Int {
    fn shr_assign(&mut self, rhs: u8) {
        self.0 >>= rhs;
    }
}

impl ShrAssign<u16> for Int {
    fn shr_assign(&mut self, rhs: u16) {
        self.0 >>= rhs;
    }
}

impl SubAssign for Int {
    fn sub_assign(&mut self, rhs: Int) {
        self.0 -= rhs.0;
    }
}

impl AddAssign<i32> for Slice {
    fn add_assign(&mut self, rhs: i32) {
        for lhs in &mut self.0 {
            *lhs += rhs;
        }
    }
}

impl<'a> AddAssign<i32> for View<'a> {
    fn add_assign(&mut self, rhs: i32) {
        for lhs in self.0.iter_mut() {
            *lhs += rhs;
        }
    }
}
