// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// aux-build:default_ty_param_cross_crate_crate.rs
// compile-flags: --error-format=human

#![feature(default_type_parameter_fallback)]

extern crate default_param_test;

use default_param_test::{Foo, bleh};

fn meh<X, B=bool>(x: Foo<X, B>) {}

fn main() {
    let foo = bleh();

    meh(foo);
}
