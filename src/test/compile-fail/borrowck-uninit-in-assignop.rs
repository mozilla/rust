// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Tests that the use of uninitialized variable in assignment operator
// expression is detected.

pub fn main() {
    let x: isize;
    x += 1; //~ ERROR use of possibly uninitialized variable: `x`

    let x: isize;
    x -= 1; //~ ERROR use of possibly uninitialized variable: `x`

    let x: isize;
    x *= 1; //~ ERROR use of possibly uninitialized variable: `x`

    let x: isize;
    x /= 1; //~ ERROR use of possibly uninitialized variable: `x`

    let x: isize;
    x %= 1; //~ ERROR use of possibly uninitialized variable: `x`

    let x: isize;
    x ^= 1; //~ ERROR use of possibly uninitialized variable: `x`

    let x: isize;
    x &= 1; //~ ERROR use of possibly uninitialized variable: `x`

    let x: isize;
    x |= 1; //~ ERROR use of possibly uninitialized variable: `x`

    let x: isize;
    x <<= 1;    //~ ERROR use of possibly uninitialized variable: `x`

    let x: isize;
    x >>= 1;    //~ ERROR use of possibly uninitialized variable: `x`
}
