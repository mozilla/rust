// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::ops::Deref;

struct DerefArray<'a, T:'a> {
    inner: &'a [T]
}

impl<'a, T> Deref for DerefArray<'a, T> {
    type Target = &'a [T];

    fn deref<'b>(&'b self) -> &'b &'a [T] {
        &self.inner
    }
}

pub fn main() {
    let a = &[1i, 2i, 3i];
    assert_eq!(DerefArray {inner: a}[1], 2);
}
