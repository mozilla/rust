// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn foo(a: i32, b: i32, c: i32, d: i32) { }

fn main() {

          foo(1000, 1000, 1000);
// EXPECT ^~~~~~~~~~~~~~~~~~~~~


// EXPECT ,~~~~~~~~~~~~~~
          foo(1000, 1000,
              1000);

// EXPECT        ,~~~~~~~~~~~~~~
                 foo(1000, 1000,
          1000);

// EXPECT    ,~~~~~~~~~~~~~~
             foo(1000, 1000,
          1000);

// EXPECT     ,~~~~~~~~~~~~~~
              foo(1000, 1000,
          1000);

// EXPECT     ,~~~~~~~~
              foo(1000,
                        1000,
                              1000);

// EXPECT ,~~~~~~~~
          foo(1000,
              1000,
              1000,
              1000,
              1000,
              1000,
              1000);

// The above is elided because it is too many lines but prints the
// overhead squigglies

}
