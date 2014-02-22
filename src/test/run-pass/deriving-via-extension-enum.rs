// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[deriving(Eq)]
enum Foo {
    Bar(int, int),
    Baz(f64, f64)
}

pub fn main() {
    let a = Bar(1, 2);
    let b = Bar(1, 2);
    fail_unless_eq!(a, b);
    fail_unless!(!(a != b));
    fail_unless!(a.eq(&b));
    fail_unless!(!a.ne(&b));
}
