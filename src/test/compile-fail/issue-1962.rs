// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-flags: -D while-true
fn main() {
  let mut i = 0is;
  while true  { //~ ERROR denote infinite loops with loop
    i += 1is;
    if i == 5is { break; }
  }
}
