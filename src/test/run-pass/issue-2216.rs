// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub fn main() {
    let mut x = 0i;

    'foo: loop {
        'bar: loop {
            'quux: loop {
                if 1i == 2 {
                    break 'foo;
                }
                else {
                    break 'bar;
                }
            }
            continue 'foo;
        }
        x = 42;
        break;
    }

    println!("{}", x);
    assert_eq!(x, 42);
}
