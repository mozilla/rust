// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// note that these aux-build directives must be in this order
// aux-build:svh-a-base.rs
// aux-build:svh-b.rs
// aux-build:svh-a-change-trait-bound.rs

extern crate a;
extern crate b; //~ ERROR: found possibly newer version of crate `a` which `b` depends on
//~^ NOTE: perhaps this crate needs to be recompiled

fn main() {
    b::foo()
}
