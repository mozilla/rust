// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn with_int<F>(f: F) where F: FnOnce(&isize) {
    let x = 3;
    f(&x);
}

fn main() {
    let mut x: Option<&isize> = None;
    with_int(|y| x = Some(y));   //~ ERROR cannot infer
}
