// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct a {
    b: usize,
    c: usize
}

impl a {
    fn foo(&self) {
        let a { x, y } = self.d; //~ ERROR attempted access of field `d`
        //~^ ERROR struct `a` does not have a field named `x`
        //~^^ ERROR struct `a` does not have a field named `y`
        //~^^^ ERROR pattern does not mention field `b`
        //~^^^^ ERROR pattern does not mention field `c`
    }
}

fn main() {}
