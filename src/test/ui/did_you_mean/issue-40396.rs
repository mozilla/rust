// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn foo() {
    println!("{:?}", (0..13).collect<Vec<i32>>()); // ok
}

fn bar() {
    println!("{:?}", Vec<i32>::new()); // ok
}

fn qux() {
    println!("{:?}", (0..13).collect<Vec<i32>()); //~ ERROR expected function, found struct `Vec`
    //~^ ERROR attempted to take value of method `collect`
}

fn main() {}
