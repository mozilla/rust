// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(optin_builtin_traits)]

struct TestType;

trait TestTrait {
    fn dummy(&self) { }
}

impl !TestTrait for TestType {}
//~^ ERROR negative impls are only allowed for traits with default impls (e.g., `Send` and `Sync`)

fn main() {}
