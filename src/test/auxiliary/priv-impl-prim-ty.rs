// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub trait A {
    fn frob(&self);
}

impl A for int { fn frob(&self) {} }

pub fn frob<T:A>(t: T) {
    t.frob();
}
