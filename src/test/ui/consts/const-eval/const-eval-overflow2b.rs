#![allow(unused_imports)]

// Note: the relevant lint pass here runs before some of the constant
// evaluation below (e.g. that performed by codegen and llvm), so if you
// change this warn to a deny, then the compiler will exit before
// those errors are detected.

#![deny(const_err)]

use std::fmt;
use std::{i8, i16, i32, i64, isize};
use std::{u8, u16, u32, u64, usize};

const VALS_I8: (i8,) =
     //~^ ERROR this constant cannot be used
    (
     i8::MAX + 1,
     );

const VALS_I16: (i16,) =
     //~^ ERROR this constant cannot be used
    (
     i16::MAX + 1,
     );

const VALS_I32: (i32,) =
     //~^ ERROR this constant cannot be used
    (
     i32::MAX + 1,
     );

const VALS_I64: (i64,) =
     //~^ ERROR this constant cannot be used
    (
     i64::MAX + 1,
     );

const VALS_U8: (u8,) =
     //~^ ERROR this constant cannot be used
    (
     u8::MAX + 1,
     );

const VALS_U16: (u16,) = (
     //~^ ERROR this constant cannot be used
     u16::MAX + 1,
     );

const VALS_U32: (u32,) = (
     //~^ ERROR this constant cannot be used
     u32::MAX + 1,
     );

const VALS_U64: (u64,) =
     //~^ ERROR this constant cannot be used
    (
     u64::MAX + 1,
     );

fn main() {
    foo(VALS_I8);
    foo(VALS_I16);
    foo(VALS_I32);
    foo(VALS_I64);

    foo(VALS_U8);
    foo(VALS_U16);
    foo(VALS_U32);
    foo(VALS_U64);
}

fn foo<T>(_: T) {
}
