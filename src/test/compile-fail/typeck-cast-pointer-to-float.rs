// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    let x : i16 = 22;
    ((&x) as *const i16) as f32;
    //~^ ERROR: cannot cast from pointer to float directly: `*const i16` as `f32`
}
