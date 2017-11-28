// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(default_type_parameter_fallback)]

use std::fmt::Debug;

trait B {
    fn b(&self) -> Self;
}

// Strangely, having a default for `Self`
// will fail for fundamental types such as `T` or `Box<T>`.
// Replace `Option` for `Box` in this test to reproduce.
impl<T=String> B for Option<T>
    where T: Default
{
    fn b(&self) -> Option<T> {
        Some(T::default())
    }
}

fn main() {
    let x = None;
    foo(x.b());
}

fn foo<T=String>(a: Option<T>)
    where T: Debug + PartialEq<&'static str> {
    assert_eq!(a.unwrap(), "");
}
