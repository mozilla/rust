// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that labels injected by macros do not break hygiene.

// Issue #24278: The label/lifetime shadowing checker from #24162
// conservatively ignores hygiene, and thus issues warnings that are
// both true- and false-positives for this test.

macro_rules! loop_x {
    ($e: expr) => {
        // $e shouldn't be able to interact with this 'x
        'x: loop { $e }
    }
}

macro_rules! run_once {
    ($e: expr) => {
        // ditto
        'x: for _ in 0..1 { $e }
    }
}

macro_rules! while_x {
    ($e: expr) => {
        // ditto
        'x: while 1 + 1 == 2 { $e }
    }
}

pub fn main() {
    'x: for _ in 0..1 {
        // this 'x should refer to the outer loop, lexically
        loop_x!(break 'x);
        panic!("break doesn't act hygienically inside for loop");
    }

    'x: loop {
        // ditto
        loop_x!(break 'x);
        panic!("break doesn't act hygienically inside infinite loop");
    }

    'x: while 1 + 1 == 2 {
        while_x!(break 'x);
        panic!("break doesn't act hygienically inside infinite while loop");
    }

    'x: for _ in 0..1 {
        // ditto
        run_once!(continue 'x);
        panic!("continue doesn't act hygienically inside for loop");
    }
}
