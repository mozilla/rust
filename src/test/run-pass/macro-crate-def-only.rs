// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:macro_crate_def_only.rs

#[macro_use] #[no_link]
extern crate macro_crate_def_only;

pub fn main() {
    assert_eq!(5i, make_a_5!());
}
