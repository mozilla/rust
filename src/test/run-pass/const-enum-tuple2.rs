// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum E { V0, V16(u16) }
static C: (E, u16, u16) = (E::V0, 0x600D, 0xBAD);

pub fn main() {
    let (_, n, _) = C;
    assert!(n != 0xBAD);
    assert_eq!(n, 0x600D);
}
