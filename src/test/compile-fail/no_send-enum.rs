// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(optin_builtin_traits)]

use std::marker::Send;

struct NoSend;
impl !Send for NoSend {}

enum Foo {
    A(NoSend)
}

fn bar<T: Send>(_: T) {}

fn main() {
    let x = Foo::A(NoSend);
    bar(x);
    //~^ ERROR `core::marker::Send` is not implemented
}
