// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[derive(Debug)]
struct Unit;

#[derive(Debug)]
struct Tuple(isize, usize);

#[derive(Debug)]
struct Struct { x: isize, y: usize }

#[derive(Debug)]
enum Enum {
    Nullary,
    Variant(isize, usize),
    StructVariant { x: isize, y : usize }
}

macro_rules! t {
    ($x:expr, $expected:expr) => {
        assert_eq!(format!("{:?}", $x), $expected.to_string())
    }
}

pub fn main() {
    t!(Unit, "Unit");
    t!(Tuple(1, 2), "Tuple(1, 2)");
    t!(Struct { x: 1, y: 2 }, "Struct { x: 1, y: 2 }");
    t!(Enum::Nullary, "Nullary");
    t!(Enum::Variant(1, 2), "Variant(1, 2)");
    t!(Enum::StructVariant { x: 1, y: 2 }, "StructVariant { x: 1, y: 2 }");
}
