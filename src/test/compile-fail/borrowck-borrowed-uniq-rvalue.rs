// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//buggy.rs

#![feature(box_syntax)]

extern crate collections;
use std::collections::HashMap;

fn main() {
    let mut buggy_map: HashMap<usize, &usize> = HashMap::new();
    buggy_map.insert(42, &*box 1); //~ ERROR borrowed value does not live long enough

    // but it is ok if we use a temporary
    let tmp = box 2;
    buggy_map.insert(43, &*tmp);
}
