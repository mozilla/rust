// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


fn f(y: Box<isize>) {
    *y = 5; //~ ERROR cannot assign
}

fn g() {
    let _frob = |&: q: Box<isize>| { *q = 2; }; //~ ERROR cannot assign

}

fn main() {}
