// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn foo(_: &'static [&'static str]) {}
fn bar(_: &'static [&'static str; 3]) {}

struct Foo {
    a: usize,
    b: u32,
}

fn main() {
    foo(&["a", "b", "c"]);
    bar(&["d", "e", "f"]);
}

trait Trait {
    const INT: usize;
}

fn generic<T: Trait>() -> &'static Foo {
    &(Foo {a: T::INT, b: 42})
}

fn generic2<T: Trait>() -> &'static usize {
    &T::INT
}
