// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub trait Foo<T> {
    fn func1<U>(&self, t: U);

    fn func2<U>(&self, t: U) {
        self.func1(t);
    }
}

pub fn main() {

}
