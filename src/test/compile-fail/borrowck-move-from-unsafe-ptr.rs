// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


fn foo(x: *const Box<isize>) -> Box<isize> {
    let y = *x; //~ ERROR dereference of unsafe pointer requires unsafe function or block
    return y;
}

fn main() {
}
