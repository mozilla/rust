// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn asBlock<F>(f: F) -> uint where F: FnOnce() -> uint {
   return f();
}

pub fn main() {
   let x = asBlock(|| 22u);
   assert_eq!(x, 22u);
}
