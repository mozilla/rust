// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


struct parser {
    tokens: Vec<isize> ,
}

trait parse {
    fn parse(&self) -> Vec<isize> ;
}

impl parse for parser {
    fn parse(&self) -> Vec<isize> {
        self.tokens //~ ERROR cannot move out of borrowed content
    }
}

fn main() {}
