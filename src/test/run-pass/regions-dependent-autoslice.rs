// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test lifetimes are linked properly when we autoslice a vector.
// Issue #3148.


fn subslice1<'r>(v: &'r [uint]) -> &'r [uint] { v }

fn both<'r>(v: &'r [uint]) -> &'r [uint] {
    subslice1(subslice1(v))
}

pub fn main() {
    let v = vec!(1,2,3);
    both(v.as_slice());
}
