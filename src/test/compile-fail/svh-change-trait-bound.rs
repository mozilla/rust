// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// note that these aux-build directives must be in this order
// aux-build:svh_a_base.rs
// aux-build:svh_b.rs
// aux-build:svh_a_change_trait_bound.rs

#![feature(macro_rules)]

extern crate a;
extern crate b; //~ ERROR: found possibly newer version of crate `a` which `b` depends on
//~^ NOTE: perhaps this crate needs to be recompiled

fn main() {
    b::foo()
}
