// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(unboxed_closures)]

use std::ops::{FnMut};

struct S {
    x: int,
    y: int,
}

impl FnMut<(),int> for S {
    extern "rust-call" fn call_mut(&mut self, (): ()) -> int {
        self.x * self.y
    }
}

fn main() {
    let mut s = S {
        x: 3,
        y: 3,
    };
    let ans = s();
    assert_eq!(ans, 9);
}


