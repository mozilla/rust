// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unknown_features)]
#![feature(box_syntax)]

use std::thread::Thread;

struct Pair {
    a: int,
    b: int
}

pub fn main() {
    let z = box Pair { a : 10, b : 12};

    let _t = Thread::spawn(move|| {
        assert_eq!(z.a, 10);
        assert_eq!(z.b, 12);
    });
}
