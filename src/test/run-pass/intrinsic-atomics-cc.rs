// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:cci_intrinsic.rs

extern crate cci_intrinsic;
use cci_intrinsic::atomic_xchg;

pub fn main() {
    let mut x = 1;
    atomic_xchg(&mut x, 5);
    assert_eq!(x, 5);
}
