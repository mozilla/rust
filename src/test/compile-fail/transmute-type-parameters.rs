// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Tests that `transmute` cannot be called on type parameters.

use std::mem::transmute;

unsafe fn f<T>(x: T) {
    let _: isize = transmute(x);  //~ ERROR cannot transmute
}

unsafe fn g<T>(x: (T, isize)) {
    let _: isize = transmute(x);  //~ ERROR cannot transmute
}

unsafe fn h<T>(x: [T; 10]) {
    let _: isize = transmute(x);  //~ ERROR cannot transmute
}

struct Bad<T> {
    f: T,
}

unsafe fn i<T>(x: Bad<T>) {
    let _: isize = transmute(x);  //~ ERROR cannot transmute
}

enum Worse<T> {
    A(T),
    B,
}

unsafe fn j<T>(x: Worse<T>) {
    let _: isize = transmute(x);  //~ ERROR cannot transmute
}

unsafe fn k<T>(x: Option<T>) {
    let _: isize = transmute(x);  //~ ERROR cannot transmute
}

fn main() {}
