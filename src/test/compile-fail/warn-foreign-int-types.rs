// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![forbid(improper_ctypes)]
#![allow(dead_code)]

mod xx {
    extern {
        pub fn strlen(str: *const u8) -> usize; //~ ERROR found rust type `usize`
        pub fn foo(x: isize, y: usize); //~ ERROR found rust type `isize`
        //~^ ERROR found rust type `usize`
    }
}

fn main() {
}
