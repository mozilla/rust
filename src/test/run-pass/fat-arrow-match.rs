// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


enum color {
    red,
    green,
    blue
}

pub fn main() {
    println!("{}", match color::red {
        color::red => { 1i }
        color::green => { 2i }
        color::blue => { 3i }
    });
}
