// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[allow(default_methods)];

trait Foo {
    fn a(&self) -> int;
    fn b(&self) -> int {
        self.a() + 2
    }
}

impl Foo for int {
    fn a(&self) -> int {
        3
    }
}

pub fn main() {
    fail_unless!((3.b() == 5));
}
