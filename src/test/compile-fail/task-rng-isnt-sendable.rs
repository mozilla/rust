// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ensure that the ThreadRng isn't/doesn't become accidentally sendable.

use std::rand;

fn test_send<S: Send>() {}

pub fn main() {
    test_send::<rand::ThreadRng>();
    //~^ ERROR `core::marker::Send` is not implemented
}
