// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that we can compile code that uses a `_` in function argument
// patterns.

fn foo((x, _): (int, int)) -> int {
    x
}

pub fn main() {
    assert_eq!(foo((22, 23)), 22);
}
