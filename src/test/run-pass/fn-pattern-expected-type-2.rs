// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub fn main() {
    let v : &[(int,int)] = &[ (1, 2), (3, 4), (5, 6) ];
    for &(x, y) in v.iter() {
        println!("{}", y);
        println!("{}", x);
    }
}
