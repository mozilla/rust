// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum E { V1(int), V0 }
static C: &'static [E] = &[E::V0, E::V1(0xDEADBEE), E::V0];

pub fn main() {
    match C[1] {
        E::V1(n) => assert!(n == 0xDEADBEE),
        _ => panic!()
    }
    match C[2] {
        E::V0 => (),
        _ => panic!()
    }
}
