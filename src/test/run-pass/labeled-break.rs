// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub fn main() {
    'foo: loop {
        loop {
            break 'foo;
        }
    }

    'bar: for _ in range(0i, 100i) {
        loop {
            break 'bar;
        }
    }

    'foobar: while 1i + 1 == 2 {
        loop {
            break 'foobar;
        }
    }
}
