// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Check that unsafe traits require unsafe impls and that inherent
// impls cannot be unsafe.

trait SafeTrait {
    fn foo(self) { }
}

unsafe trait UnsafeTrait {
    fn foo(self) { }
}

unsafe impl UnsafeTrait for u8 { } // OK

impl UnsafeTrait for u16 { } //~ ERROR requires an `unsafe impl` declaration

unsafe impl SafeTrait for u32 { } //~ ERROR the trait `SafeTrait` is not unsafe

fn main() { }
