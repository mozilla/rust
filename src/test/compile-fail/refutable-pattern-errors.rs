// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


fn func((1, (Some(1), 2...3)): (isize, (Option<isize>, isize))) { }
//~^ ERROR refutable pattern in function argument: `(_, _)` not covered

fn main() {
    let (1is, (Some(1is), 2is...3is)) = (1is, (None, 2is));
    //~^ ERROR refutable pattern in local binding: `(_, _)` not covered
}
