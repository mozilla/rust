// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(type_limits)]

pub fn main() {
    let i: uint = 0;
    assert!(i <= 0xFFFF_FFFF_u);

    let i: int = 0;
    assert!(i >= -0x8000_0000_i);
    assert!(i <= 0x7FFF_FFFF_i);
}
