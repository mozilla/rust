// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]

fn main() {
    let f;
    let g;
    g = f;
    f = box g;
    //~^  ERROR mismatched types
    //~| expected `_`
    //~| found `Box<_>`
    //~| cyclic type of infinite size
}
