// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    let a = "".to_string();
    let b: Vec<&str> = a.as_slice().lines().collect();
    drop(a);    //~ ERROR cannot move out of `a` because it is borrowed
    for s in b.iter() {
        println!("{}", *s);
    }
}

