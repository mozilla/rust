// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


fn swap<F>(f: F) -> Vec<int> where F: FnOnce(Vec<int>) -> Vec<int> {
    let x = vec!(1, 2, 3);
    f(x)
}

pub fn main() {
    let v = swap(|mut x| { x.push(4); x });
    let w = swap(|mut x| { x.push(4); x });
    assert_eq!(v, w);
}
