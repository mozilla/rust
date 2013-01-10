// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct Bar {
    x: int,
}

impl Bar : Drop {
    fn finalize(&self) {
        io::println("Goodbye, cruel world");
    }
}

struct Foo {
    x: int,
    y: Bar
}

fn main() {
    let a = Foo { x: 1, y: Bar { x: 5 } };
    let c = Foo { x: 4, .. a}; //~ ERROR cannot copy field `y` of base expression, which has a noncopyable type
    io::println(fmt!("%?", c));
}

