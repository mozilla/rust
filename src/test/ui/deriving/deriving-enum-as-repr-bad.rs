// Test that AsRepr cannot be derived other than for enums with an explicit int repr and no data.

// gate-test-enum_as_repr
// The trait is auto-derived in stage2
// ignore-stage2

#![feature(enum_as_repr)]
#![allow(unused)]

use std::enums::AsRepr;

#[derive(AsRepr)]
//~^ ERROR `AsRepr` can only be derived for enums [FIXME]
struct Struct {}

#[derive(AsRepr)]
//~^ ERROR `AsRepr` can only be derived for enums with an explicit integer representation [FIXME]
#[repr(C)]
enum NumberC {
    Zero,
    One,
}

#[derive(AsRepr)]
//~^ ERROR `AsRepr` can only be derived for enums with an explicit integer representation [FIXME]
enum NumberNoRepr {
    Zero,
    One,
}

#[derive(AsRepr)]
//~^ ERROR `AsRepr` can only be derived for data-free enums [FIXME]
#[repr(u8)]
enum NumberTuple {
    Zero,
    NonZero(u8),
}

#[derive(AsRepr)]
//~^ ERROR `AsRepr` can only be derived for data-free enums [FIXME]
#[repr(u8)]
enum NumberStruct {
    Zero,
    NonZero { value: u8 },
}

#[derive(AsRepr)]
//~^ ERROR `AsRepr` can only be derived for data-free enums [FIXME]
#[repr(u8)]
enum NumberEmptyTuple {
    Zero(),
    NonZero,
}

#[derive(AsRepr)]
//~^ ERROR `AsRepr` can only be derived for data-free enums [FIXME]
#[repr(u8)]
enum NumberEmptyStruct {
    Zero {},
    NonZero,
}

fn main() {}
