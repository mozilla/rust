// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


struct Bar<'a> {
    f: &'a isize,
}

impl<'a> Drop for Bar<'a> {
//~^ ERROR E0141
    fn drop(&mut self) {
    }
}

struct Baz {
    f: &'static isize,
}

impl Drop for Baz {
    fn drop(&mut self) {
    }
}

fn main() { }
