// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(dead_assignment)]
#![allow(unreachable_code)]
#![allow(unused_variable)]

fn test(_cond: bool) {
    let v: int;
    v = 1;
    loop { } // loop never terminates, so no error is reported
    v = 2;
}

pub fn main() {
    // note: don't call test()... :)
}
