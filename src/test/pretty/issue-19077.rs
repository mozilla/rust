// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Testing that unsafe blocks in match arms are followed by a comma
// pp-exact
fn main() {
    match true {
        true if true => (),
        false if false => unsafe { },
        true => { }
        false => (),
    }
}
