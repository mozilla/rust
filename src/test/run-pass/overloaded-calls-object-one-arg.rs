// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Tests calls to closure arguments where the closure takes 1 argument.
// This is a bit tricky due to rust-call ABI.

fn foo(f: &mut FnMut(int) -> int) -> int {
    f(22)
}

fn main() {
    let z = foo(&mut |x| x *100);
    assert_eq!(z, 2200);
}
