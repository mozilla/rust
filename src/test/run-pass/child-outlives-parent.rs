// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Reported as issue #126, child leaks the string.

use std::thread::Thread;

fn child2(_s: String) { }

pub fn main() {
    let _x = Thread::spawn(move|| child2("hi".to_string()));
}
