// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn ho<F>(f: F) -> int where F: FnOnce(int) -> int { let n: int = f(3); return n; }

fn direct(x: int) -> int { return x + 1; }

pub fn main() {
    let a: int = direct(3); // direct
    let b: int = ho(direct); // indirect unbound

    assert_eq!(a, b);
}
