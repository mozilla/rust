// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn foo<T>(y: Option<T>) {
    let mut x: int;
    let mut rs: Vec<int> = Vec::new();
    /* tests that x doesn't get put in the precondition for the
       entire if expression */

    if true {
    } else {
        match y {
          None::<T> => x = 17,
          _ => x = 42
        }
        rs.push(x);
    }
    return;
}

pub fn main() { println!("hello"); foo::<int>(Some::<int>(5)); }
