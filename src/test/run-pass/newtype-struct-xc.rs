// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:newtype_struct_xc.rs

extern crate newtype_struct_xc;

pub fn main() {
    let _ = newtype_struct_xc::Au(2);
}
