// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// no-combine - Cannot be combined due to use of type_name

#![feature(core_intrinsics)]

use std::intrinsics::type_name;

struct Foo<T> {
    x: T
}

pub fn main() {
    unsafe {
        assert_eq!(type_name::<isize>(), "isize");
        assert_eq!(type_name::<Foo<usize>>(), "Foo<usize>");
    }
}
