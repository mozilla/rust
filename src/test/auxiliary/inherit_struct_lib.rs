// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test struct inheritance on structs from another crate.
#![feature(struct_inherit)]

pub virtual struct S1 {
    pub f1: int,
}

pub struct S2 : S1 {
    pub f2: int,
}

pub fn test_s2(s2: S2) {
    assert!(s2.f1 == 115);
    assert!(s2.f2 == 113);
}

pub static glob_s: S2 = S2 { f1: 32, f2: -45 };
