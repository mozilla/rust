// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:overloaded_autoderef_xc.rs

extern crate overloaded_autoderef_xc;

fn main() {
    assert!(overloaded_autoderef_xc::check(5i, 5i));
}
