// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// When all branches of an if expression result in panic, the entire if
// expression results in panic.
pub fn main() {
    let _x = if true {
        10i
    } else {
        if true { panic!() } else { panic!() }
    };
}
