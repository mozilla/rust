// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::iter::AdditiveIterator;
fn main() {
    let x: [u64; 3] = [1, 2, 3];
    assert_eq!(6, range(0, 3).map(|i| x[i]).sum());
}
