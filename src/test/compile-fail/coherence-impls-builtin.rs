// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::marker::Send;

enum TestE {
  A
}

struct MyType;

unsafe impl Send for TestE {}
unsafe impl Send for MyType {}
unsafe impl Send for (MyType, MyType) {}
//~^ ERROR builtin traits can only be implemented on structs or enums

unsafe impl Send for &'static MyType {}
//~^ ERROR builtin traits can only be implemented on structs or enums

unsafe impl Send for [MyType] {}
//~^ ERROR builtin traits can only be implemented on structs or enums

unsafe impl Send for &'static [MyType] {}
//~^ ERROR builtin traits can only be implemented on structs or enums

fn is_send<T: Send>() {}

fn main() {
    is_send::<(MyType, TestE)>();
}
