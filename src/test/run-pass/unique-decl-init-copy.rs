// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unknown_features)]
#![feature(box_syntax)]

pub fn main() {
    let mut i = box 1i;
    // Should be a copy
    let mut j = i.clone();
    *i = 2i;
    *j = 3i;
    assert_eq!(*i, 2i);
    assert_eq!(*j, 3i);
}
