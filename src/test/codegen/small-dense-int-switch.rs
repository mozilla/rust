// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[no_mangle]
pub fn test(x: int, y: int) -> int {
    match x {
        1 => y,
        2 => y*2,
        4 => y*3,
        _ => 11
    }
}
