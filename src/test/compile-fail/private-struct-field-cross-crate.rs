// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:cci_class.rs
extern crate cci_class;
use cci_class::kitties::cat;

fn main() {
  let nyan : cat = cat(52us, 99);
  assert!((nyan.meows == 52us));
  //~^ ERROR field `meows` of struct `cci_class::kitties::cat` is private
}
