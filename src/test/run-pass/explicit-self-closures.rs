// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test to make sure that explicit self params work inside closures

struct Box {
    x: uint
}

impl Box {
    pub fn set_many(&mut self, xs: &[uint]) {
        for x in xs.iter() { self.x = *x; }
    }
}

pub fn main() {}
