// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

trait A {
    fn foo(*mut self); //~ ERROR cannot pass self by unsafe pointer
    fn bar(*self); //~ ERROR cannot pass self by unsafe pointer
}

struct X;
impl A for X {
    fn foo(*mut self) { } //~ ERROR cannot pass self by unsafe pointer
    fn bar(*self) { } //~ ERROR cannot pass self by unsafe pointer
}

fn main() { }
