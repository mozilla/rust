// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


fn main() {
    let needlesArr: Vec<char> = vec!('a', 'f');
    needlesArr.iter().fold(|&: x, y| {
    });
    //~^^ ERROR this function takes 2 parameters but 1 parameter was supplied
    //
    // the first error is, um, non-ideal.
}
