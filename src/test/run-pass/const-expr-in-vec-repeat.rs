// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Check that constant expressions can be used in vec repeat syntax.

pub fn main() {

    const FOO: uint = 2;
    let _v = [0i; FOO*3*2/2];

}
