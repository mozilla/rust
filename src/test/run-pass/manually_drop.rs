// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
extern crate core;

static mut value: uint = 0;

struct Canary;

impl Drop for Canary {
    fn drop(&mut self) {
        unsafe {
            value += 1;
        }
    }
}

fn main() {
    unsafe {
        assert_eq!(value, 0);

        { Canary; }
        assert_eq!(value, 1);

        { core::manually_drop::ManuallyDrop::new(Canary); }
        assert_eq!(value, 1);

        { core::manually_drop::ManuallyDrop::new(Canary).into_inner(); }
        assert_eq!(value, 2);
    }
}
