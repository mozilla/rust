// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Regression test for #36053. ICE was caused due to obligations
// being added to a special, dedicated fulfillment cx during
// a probe.

use std::iter::once;
fn main() {
    once::<&str>("str").fuse().filter(|a: &str| true).count();
    //~^ ERROR no method named `count`
    //~| ERROR E0281
    //~| ERROR E0281
    //~| NOTE expected &str, found str
    //~| NOTE expected &str, found str
    //~| NOTE implements
    //~| NOTE implements
    //~| NOTE requires
    //~| NOTE requires
    //~| NOTE the method `count` exists but the following trait bounds
}
