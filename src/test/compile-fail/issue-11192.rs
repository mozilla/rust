// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]

struct Foo {
    x: isize
}

impl Drop for Foo {
    fn drop(&mut self) {
        println!("drop {}", self.x);
    }
}

fn main() {
    let mut ptr = box Foo { x: 0 };
    let mut test = |&mut: foo: &Foo| {
        println!("access {}", foo.x);
        ptr = box Foo { x: ptr.x + 1 };
        println!("access {}", foo.x);
    };
    test(&*ptr);
    //~^ ERROR: cannot borrow `*ptr` as immutable
}

