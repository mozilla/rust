// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cell::RefCell;

fn f<T: Sync>(_: T) {}

fn main() {
    let x = RefCell::new(0is);
    f(x);
    //~^ ERROR `core::marker::Sync` is not implemented
    //~^^ ERROR `core::marker::Sync` is not implemented
}
