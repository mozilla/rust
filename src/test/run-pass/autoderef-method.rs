// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unknown_features)]
#![feature(box_syntax)]

trait double {
    fn double(self: Box<Self>) -> uint;
}

impl double for uint {
    fn double(self: Box<uint>) -> uint { *self * 2u }
}

pub fn main() {
    let x = box 3u;
    assert_eq!(x.double(), 6u);
}
