// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that bounds are sized-compatible.

trait T : Sized {}
fn f<Y: ?Sized + T>() {
//~^ERROR incompatible bounds on `Y`, bound `T` does not allow unsized type
}

pub fn main() {
}
