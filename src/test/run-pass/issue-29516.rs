// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(immovable_types)]
#![feature(optin_builtin_traits)]

use std::marker::Move;

trait NotSame: ?Move {}
#[allow(auto_impl)]
impl NotSame for .. {}
impl<A> !NotSame for (A, A) {}

trait OneOfEach {}

impl<A> OneOfEach for (A,) {}

impl<A, B> OneOfEach for (A, B)
where
    (B,): OneOfEach,
    (A, B): NotSame,
{
}

fn main() {}
