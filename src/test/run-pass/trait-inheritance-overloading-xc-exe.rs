// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:trait_inheritance_overloading_xc.rs

extern crate trait_inheritance_overloading_xc;
use trait_inheritance_overloading_xc::{MyNum, MyInt};

fn f<T:MyNum>(x: T, y: T) -> (T, T, T) {
    return (x.clone() + y.clone(), x.clone() - y.clone(), x * y);
}

fn mi(v: int) -> MyInt { MyInt { val: v } }

pub fn main() {
    let (x, y) = (mi(3), mi(5));
    let (a, b, c) = f(x, y);
    assert_eq!(a, mi(8));
    assert_eq!(b, mi(-2));
    assert_eq!(c, mi(15));
}
