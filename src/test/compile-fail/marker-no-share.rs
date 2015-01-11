// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-stage1
// ignore-stage2
// ignore-stage3

use std::marker;

fn foo<P: Sync>(p: P) { }

fn main()
{
    foo(marker::NoSync); //~ ERROR the trait `core::marker::Sync` is not implemented
}
