// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[inline]
fn inlined() -> u32 {
    1234
}

fn normal() -> u32 {
    2345
}

mod a {
    pub fn f() -> u32 {
        ::inlined() + ::normal()
    }
}

mod b {
    pub fn f() -> u32 {
        ::inlined() + ::normal()
    }
}

fn main() {
    a::f();
    b::f();
}
