// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]

use std::thread::Thread;

fn borrow<F>(v: &isize, f: F) where F: FnOnce(&isize) {
    f(v);
}

fn box_imm() {
    let v = box 3is;
    let _w = &v;
    Thread::spawn(move|| {
        println!("v={}", *v);
        //~^ ERROR cannot move `v` into closure
    });
}

fn box_imm_explicit() {
    let v = box 3is;
    let _w = &v;
    Thread::spawn(move|| {
        println!("v={}", *v);
        //~^ ERROR cannot move
    });
}

fn main() {
}
