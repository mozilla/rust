// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_type = "dylib"]
#![allow(dead_code)]

#[no_mangle] pub extern "C" fn fun1() {}
#[no_mangle] extern "C" fn fun2() {}

mod foo {
    #[no_mangle] pub extern "C" fn fun3() {}
}
pub mod bar {
    #[no_mangle] pub extern "C" fn fun4() {}
}

#[no_mangle] pub fn fun5() {}
