// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn get_third<T>(t: (T, T, T)) -> T { let (_, _, x) = t; return x; }

pub fn main() {
    println!("{}", get_third((1i, 2i, 3i)));
    assert_eq!(get_third((1i, 2i, 3i)), 3);
    assert_eq!(get_third((5u8, 6u8, 7u8)), 7u8);
}
