// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Issue #12512.

fn main() {
    let mut foo = Vec::new();
    'foo: for i in [1i, 2, 3].iter() {
        foo.push(i);
    }
}

