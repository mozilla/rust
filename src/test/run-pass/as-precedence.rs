// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    assert_eq!(3 as uint * 3, 9);
    assert_eq!(3 as (uint) * 3, 9);
    assert_eq!(3 as (uint) / 3, 1);
    assert_eq!(3 as uint + 3, 6);
    assert_eq!(3 as (uint) + 3, 6);
}

