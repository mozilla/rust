// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-flags: --cfg ndebug
// exec-env:RUST_LOG=conditional-debug-macro-off=4

#[macro_use]
extern crate log;

pub fn main() {
    // only panics if println! evaluates its argument.
    debug!("{:?}", { if true { panic!() } });
}
