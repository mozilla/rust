// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Check that non constant exprs fail for vector repeat syntax

fn main() {
    fn bar(n: usize) {
        let _x = [0; n]; //~ ERROR expected constant integer for repeat count, found variable
    }
}
