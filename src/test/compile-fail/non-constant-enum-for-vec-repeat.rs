// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum State { ST_NULL, ST_WHITESPACE }

fn main() {
    [State::ST_NULL; (State::ST_WHITESPACE as usize)];
    //~^ ERROR expected constant integer for repeat count, found non-constant expression
}
