// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn force<F>(f: F) -> int where F: FnOnce() -> int { return f(); }

pub fn main() {
    fn f() -> int { return 7; }
    assert_eq!(force(f), 7);
    let g = {|&:|force(f)};
    assert_eq!(g(), 7);
}
