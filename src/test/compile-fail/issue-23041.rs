// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::any::Any;
fn main()
{
    fn bar(x:&'static i32) -> &'static i32 { x };
    let b:Box<Any> = Box::new(bar as fn(&'static _)->&'static _);
    b.downcast_ref::<fn(&_)->&_>(); //~ ERROR unconstrained type
}
