// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum X {
    A =
        b'a' //~ ERROR discriminator values can only be used with a c-like enum
    ,
    B(isize)
}

fn main() {}
