// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unknown_features)]
#![feature(box_syntax)]

struct Recbox<T> {x: Box<T>}

fn reclift<T>(t: T) -> Recbox<T> { return Recbox {x: box t}; }

pub fn main() {
    let foo: int = 17;
    let rbfoo: Recbox<int> = reclift::<int>(foo);
    assert_eq!(*rbfoo.x, foo);
}
