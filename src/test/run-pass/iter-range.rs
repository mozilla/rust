// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.



fn range_<F>(a: int, b: int, mut it: F) where F: FnMut(int) {
    assert!((a < b));
    let mut i: int = a;
    while i < b { it(i); i += 1; }
}

pub fn main() {
    let mut sum: int = 0;
    range_(0, 100, |x| sum += x );
    println!("{}", sum);
}
