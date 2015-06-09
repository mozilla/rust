// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// min-lldb-version: 310

// compile-flags:-g

// === GDB TESTS ===================================================================================

// gdb-command:run

// gdb-command:print *a
// gdb-check:$1 = 1
// gdb-command:print *b
// gdb-check:$2 = {__0 = 2, __1 = 3.5}


// === LLDB TESTS ==================================================================================

// lldb-command:run
// lldb-command:print *a
// lldb-check:[...]$0 = 1
// lldb-command:print *b
// lldb-check:[...]$1 = (2, 3.5)

#![allow(unused_variables)]
#![feature(box_syntax)]
#![feature(placement_in_syntax)]
// both needed for HEAP use for some reason
#![feature(core, alloc)]
#![omit_gdb_pretty_printer_section]

use std::boxed::HEAP;

fn main() {
    let a: Box<_> = box 1;
    // FIXME (#22181): Put in the new placement-in syntax once that lands.
    let b = box (HEAP) { (2, 3.5f64) };

    zzz(); // #break
}

fn zzz() { () }
