// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Make sure #1399 stays fixed

#![allow(unknown_features)]
#![feature(box_syntax)]
#![feature(unboxed_closures)]

struct A { a: Box<int> }

fn foo() -> Box<FnMut() -> int + 'static> {
    let k = box 22i;
    let _u = A {a: k.clone()};
    let result  = |&mut:| 22;
    box result
}

pub fn main() {
    assert_eq!(foo().call_mut(()), 22);
}
