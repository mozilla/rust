// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]

trait Foo {
}

impl<T:Copy> Foo for T {
}

fn take_param<T:Foo>(foo: &T) { }

fn main() {
    let x = box 3is;
    take_param(&x);
    //~^ ERROR the trait `core::marker::Copy` is not implemented
}
