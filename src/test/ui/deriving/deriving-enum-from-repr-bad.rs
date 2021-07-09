// Test that FromRepr cannot be derived other than for enums with an explicit int repr and no data.

// gate-test-enum_as_repr

#![feature(enum_as_repr)]
#![allow(unused)]

use std::enums::FromRepr;

#[derive(FromRepr)]
//~^ ERROR `FromRepr` can only be derived for enums [FIXME]
struct Struct {}

#[derive(FromRepr)]
//~^ ERROR `FromRepr` can only be derived for enums with an explicit integer representation [FIXME]
#[repr(C)]
enum NumberC {
    Zero,
    One,
}

#[derive(FromRepr)]
//~^ ERROR `FromRepr` can only be derived for enums with an explicit integer representation [FIXME]
enum NumberNoRepr {
    Zero,
    One,
}

#[derive(FromRepr)]
//~^ ERROR `FromRepr` can only be derived for data-free enums [FIXME]
#[repr(u8)]
enum NumberTuple {
    Zero,
    NonZero(u8),
}

#[derive(FromRepr)]
//~^ ERROR `FromRepr` can only be derived for data-free enums [FIXME]
#[repr(u8)]
enum NumberStruct {
    Zero,
    NonZero { value: u8 },
}

#[derive(FromRepr)]
//~^ ERROR `FromRepr` can only be derived for data-free enums [FIXME]
#[repr(u8)]
enum NumberEmptyTuple {
    Zero(),
    NonZero,
}

#[derive(FromRepr)]
//~^ ERROR `FromRepr` can only be derived for data-free enums [FIXME]
#[repr(u8)]
enum NumberEmptyStruct {
    Zero {},
    NonZero,
}

fn main() {}
