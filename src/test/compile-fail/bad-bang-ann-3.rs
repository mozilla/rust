// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Tests that a function with a ! annotation always actually fails

fn bad_bang(i: usize) -> ! {
    return 7us; //~ ERROR `return` in a function declared as diverging [E0166]
}

fn main() { bad_bang(5us); }
