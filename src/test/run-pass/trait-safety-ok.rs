// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Simple smoke test that unsafe traits can be compiled etc.

unsafe trait Foo {
    fn foo(&self) -> int;
}

unsafe impl Foo for int {
    fn foo(&self) -> int { *self }
}

fn take_foo<F:Foo>(f: &F) -> int { f.foo() }

fn main() {
    let x: int = 22;
    assert_eq!(22, take_foo(&x));
}
