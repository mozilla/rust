// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    let x: (int, &[int]) = (2i, &[1i, 2i]);
    assert_eq!(match x {
        (0, [_, _]) => 0,
        (1, _) => 1,
        (2, [_, _]) => 2,
        (2, _) => 3,
        _ => 4
    }, 2i);
}
