// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(dead_code)]
#![allow(unknown_features)]
#![feature(box_syntax)]

// this code used to cause an ICE

trait X<T> {}

struct S<T> {f: Box<X<T>+'static>,
             g: Box<X<T>+'static>}

struct F;
impl X<int> for F {}

fn main() {
  S {f: box F, g: box F};
}
