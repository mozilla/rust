// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    match 0is { 1is => () } //~ ERROR non-exhaustive patterns
    match 0is { 0is if false => () } //~ ERROR non-exhaustive patterns
}
