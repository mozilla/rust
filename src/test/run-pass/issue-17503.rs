// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    let s: &[int] = &[0, 1, 2, 3, 4];
    let ss: &&[int] = &s;
    let sss: &&&[int] = &ss;

    println!("{:?}", &s[..3]);
    println!("{:?}", &ss[3..]);
    println!("{:?}", &sss[2..4]);
}
