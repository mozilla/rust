// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test a function that takes/returns a u8.

#[link(name = "rust_test_helpers")]
extern {
    pub fn rust_dbg_extern_identity_u8(v: u8) -> u8;
}

pub fn main() {
    unsafe {
        assert_eq!(22_u8, rust_dbg_extern_identity_u8(22_u8));
    }
}
