// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn f<T>() {}

fn main() {
    false == false == false;
    //~^ ERROR: Chained comparison operators require parentheses

    false == 0 < 2;
    //~^ ERROR: Chained comparison operators require parentheses

    f<X>();
    //~^ ERROR: Chained comparison operators require parentheses
    //~^^ HELP: Use ::< instead of < if you meant to specify type arguments.
}
