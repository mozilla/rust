// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub fn main() {
    let mut sum = 0i;
    let xs = vec!(1, 2, 3, 4, 5);
    for x in xs.iter() {
        sum += *x;
    }
    assert_eq!(sum, 15);
}
