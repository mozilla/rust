// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct Foo<T> {
    x: T,
}
impl<T> Foo<T> {
    fn add(&mut self, v: Foo<T>){
        self.x += v.x;
        //~^ ERROR: binary assignment operation `+=` cannot be applied
    }
}
fn main() {}
