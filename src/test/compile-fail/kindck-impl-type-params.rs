// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Issue #14061: tests the interaction between generic implementation
// parameter bounds and trait objects.

#![feature(box_syntax)]

struct S<T>;

trait Gettable<T> {}

impl<T: Send + Copy> Gettable<T> for S<T> {}

fn f<T>(val: T) {
    let t: S<T> = S;
    let a = &t as &Gettable<T>;
    //~^ ERROR the trait `core::marker::Send` is not implemented
    //~^^ ERROR the trait `core::marker::Copy` is not implemented
}

fn g<T>(val: T) {
    let t: S<T> = S;
    let a: &Gettable<T> = &t;
    //~^ ERROR the trait `core::marker::Send` is not implemented
    //~^^ ERROR the trait `core::marker::Copy` is not implemented
}

fn foo<'a>() {
    let t: S<&'a isize> = S;
    let a = &t as &Gettable<&'a isize>;
    //~^ ERROR declared lifetime bound not satisfied
}

fn foo2<'a>() {
    let t: Box<S<String>> = box S;
    let a = t as Box<Gettable<String>>;
    //~^ ERROR the trait `core::marker::Copy` is not implemented
}

fn foo3<'a>() {
    let t: Box<S<String>> = box S;
    let a: Box<Gettable<String>> = t;
    //~^ ERROR the trait `core::marker::Copy` is not implemented
}

fn main() { }
