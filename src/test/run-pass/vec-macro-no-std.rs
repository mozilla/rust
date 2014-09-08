// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(phase)]
#![no_std]

#[phase(plugin, link)]
extern crate core;

#[phase(plugin, link)]
extern crate collections;

extern crate native;

use core::option::Some;
use collections::vec::Vec;
use collections::MutableSeq;

// Issue #16806

fn main() {
    let x: Vec<u8> = vec!(0, 1, 2);
    match x.last() {
        Some(&2) => (),
        _ => fail!(),
    }
}
