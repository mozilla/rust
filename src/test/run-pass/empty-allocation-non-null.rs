// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unknown_features)]
#![feature(box_syntax)]

pub fn main() {
    assert!(Some(box() ()).is_some());

    let xs: Box<[()]> = box [];
    assert!(Some(xs).is_some());

    struct Foo;
    assert!(Some(box Foo).is_some());

    let ys: Box<[Foo]> = box [];
    assert!(Some(ys).is_some());
}
