// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::rc::Rc;
use std::cell::RefCell;

fn bar<T: Sync>(_: T) {}

fn main() {
    let x = Rc::new(RefCell::new(5is));
    bar(x);
    //~^ ERROR the trait `core::marker::Sync` is not implemented
}
