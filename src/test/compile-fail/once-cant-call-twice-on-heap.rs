// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Testing guarantees provided by once functions.
// This program would segfault if it were legal.

#![feature(once_fns)]
extern crate sync;
use sync::Arc;

fn foo(blk: proc()) {
    blk();
    blk(); //~ ERROR use of moved value
}

fn main() {
    let x = Arc::new(true);
    foo(proc() {
        assert!(*x.get());
        drop(x);
    });
}
