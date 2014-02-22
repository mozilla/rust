// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[packed]
struct Foo {
    bar: u8,
    baz: uint
}

pub fn main() {
    let foo = Foo { bar: 1, baz: 2 };
    match foo {
        Foo {bar, baz} => {
            fail_unless_eq!(bar, 1);
            fail_unless_eq!(baz, 2);
        }
    }
}
