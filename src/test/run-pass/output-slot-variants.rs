// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(dead_assignment)]
#![allow(unused_variable)]
#![allow(unknown_features)]
#![feature(box_syntax)]

struct A { a: int, b: int }
struct Abox { a: Box<int>, b: Box<int> }

fn ret_int_i() -> int { 10 }

fn ret_ext_i() -> Box<int> { box 10 }

fn ret_int_rec() -> A { A {a: 10, b: 10} }

fn ret_ext_rec() -> Box<A> { box A {a: 10, b: 10} }

fn ret_ext_mem() -> Abox { Abox {a: box 10, b: box 10} }

fn ret_ext_ext_mem() -> Box<Abox> { box Abox{a: box 10, b: box 10} }

pub fn main() {
    let mut int_i: int;
    let mut ext_i: Box<int>;
    let mut int_rec: A;
    let mut ext_rec: Box<A>;
    let mut ext_mem: Abox;
    let mut ext_ext_mem: Box<Abox>;
    int_i = ret_int_i(); // initializing

    int_i = ret_int_i(); // non-initializing

    int_i = ret_int_i(); // non-initializing

    ext_i = ret_ext_i(); // initializing

    ext_i = ret_ext_i(); // non-initializing

    ext_i = ret_ext_i(); // non-initializing

    int_rec = ret_int_rec(); // initializing

    int_rec = ret_int_rec(); // non-initializing

    int_rec = ret_int_rec(); // non-initializing

    ext_rec = ret_ext_rec(); // initializing

    ext_rec = ret_ext_rec(); // non-initializing

    ext_rec = ret_ext_rec(); // non-initializing

    ext_mem = ret_ext_mem(); // initializing

    ext_mem = ret_ext_mem(); // non-initializing

    ext_mem = ret_ext_mem(); // non-initializing

    ext_ext_mem = ret_ext_ext_mem(); // initializing

    ext_ext_mem = ret_ext_ext_mem(); // non-initializing

    ext_ext_mem = ret_ext_ext_mem(); // non-initializing

}
