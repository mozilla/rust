// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unknown_features)]
#![feature(box_syntax)]

struct X { x: int, y: int, z: int }

pub fn main() { let x = box X {x: 1, y: 2, z: 3}; let y = x; assert!((y.y == 2)); }
