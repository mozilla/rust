// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// This file was auto-generated using 'src/etc/generate-deriving-span-tests.py'

extern crate rand;

#[derive(PartialEq)]
struct Error;

#[derive(Eq,PartialEq)]
enum Enum {
   A {
     x: Error //~ ERROR
   }
}

fn main() {}
