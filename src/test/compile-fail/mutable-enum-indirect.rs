// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-test

// Tests that an `&` pointer to something inherently mutable is itself
// to be considered mutable.

use std::kinds::marker;

enum Foo { A(marker::NoShare) }

trait Test {}

impl<'a> Test for &'a Foo {}

fn bar<T: Test+Share>(_: T) {}

fn main() {
    let x = A(marker::NoShare);
    bar(&x);
    //FIXME(flaper87): Not yet
    // ERROR type parameter with an incompatible type
}
