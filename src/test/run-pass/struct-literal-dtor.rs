// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct foo {
    x: String,
}

impl Drop for foo {
    fn drop(&mut self) {
        println!("{}", self.x);
    }
}

pub fn main() {
    let _z = foo {
        x: "Hello".to_string()
    };
}
