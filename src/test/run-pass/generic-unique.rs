// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unknown_features)]
#![feature(box_syntax)]

struct Triple<T> { x: T, y: T, z: T }

fn box_it<T>(x: Triple<T>) -> Box<Triple<T>> { return box x; }

pub fn main() {
    let x: Box<Triple<int>> = box_it::<int>(Triple{x: 1, y: 2, z: 3});
    assert_eq!(x.y, 2);
}
