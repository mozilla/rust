// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct A<'a, 'b> where 'a : 'b { x: &'a int, y: &'b int }

fn main() {
    let x = 1i;
    let y = 1i;
    let a = A { x: &x, y: &y };
}
