// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]

trait Foo { }

impl<'a> Foo for &'a isize { }

fn main() {
    let blah;
    {
        let ss: &isize = &1; //~ ERROR borrowed value does not live long enough
        blah = box ss as Box<Foo>;
    }
}
