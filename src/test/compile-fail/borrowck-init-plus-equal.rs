// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn test() {
    let mut v: isize;
    v = v + 1; //~ ERROR use of possibly uninitialized variable: `v`
    v.clone();
}

fn main() {
}
