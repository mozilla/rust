// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

impl<'a> Drop for &'a mut isize {
    //~^ ERROR the Drop trait may only be implemented on structures
    //~^^ ERROR E0117
    fn drop(&mut self) {
        println!("kaboom");
    }
}

fn main() {
}
