// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[link(package_id = "boot", name = "boot", vers = "0.1")];

extern mod rustuv; // pull in uvio

use std::rt;
use std::ptr;

#[no_mangle] // this needs to get called from C
pub extern "C" fn foo(argc: int, argv: **u8) -> int {
    do rt::start(argc, argv) {
        do spawn {
            println!("hello");
        }
    }
}
