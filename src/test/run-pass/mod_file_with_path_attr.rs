// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-pretty

// Testing that a plain .rs file can load modules from other source files

#[path = "mod_file_aux.rs"]
mod m;

pub fn main() {
    assert_eq!(m::foo(), 10);
}
