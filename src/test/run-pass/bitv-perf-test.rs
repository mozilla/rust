//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unknown_features)]
#![feature(box_syntax)]

extern crate collections;
use std::collections::Bitv;

fn bitv_test() {
    let mut v1 = box Bitv::from_elem(31, false);
    let v2 = box Bitv::from_elem(31, true);
    v1.union(&*v2);
}

pub fn main() {
    for _ in range(0i, 10000) { bitv_test(); }
}
