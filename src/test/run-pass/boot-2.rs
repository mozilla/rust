// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[no_boot];

#[boot] extern mod green;

use std::rt::task::Task;
use std::rt::local::Local;
use std::rt::Runtime;

fn main() {
    let mut t = Local::borrow(None::<Task>);
    match t.get().maybe_take_runtime::<green::task::GreenTask>() {
        Some(rt) => {
            t.get().put_runtime(rt as ~Runtime);
        }
        None => fail!()
    }
}


