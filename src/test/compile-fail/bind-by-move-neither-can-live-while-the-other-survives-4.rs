// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct X { x: (), }

impl Drop for X {
    fn drop(&mut self) {
        println!("destructor runs");
    }
}

fn main() {
    let x = Some((X { x: () }, X { x: () }));
    match x {
        Some((_y, ref _z)) => { }, //~ ERROR cannot bind by-move and by-ref in the same pattern
        None => panic!()
    }
}
