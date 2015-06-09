// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    use std::boxed::HEAP;

    let x: Box<_> = box 'c'; //~ ERROR box expression syntax is experimental
    println!("x: {}", x);

    let x: Box<_> = box () 'c'; //~ ERROR box expression syntax is experimental
    println!("x: {}", x);

    let x: Box<_> = box (HEAP) 'c'; //~ ERROR placement-in expression syntax is experimental
    println!("x: {}", x);

    // FIXME (#22181) put back when new placement-in syntax is supported
    // let x = in HEAP { 'c' };
    // println!("x: {}", x);
}
