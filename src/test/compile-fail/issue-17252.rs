// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

static FOO: usize = FOO; //~ ERROR recursive constant

fn main() {
    let _x: [u8; FOO]; // caused stack overflow prior to fix
    let _y: usize = 1 + {
        static BAR: usize = BAR; //~ ERROR recursive constant
        let _z: [u8; BAR]; // caused stack overflow prior to fix
        1
    };
}
