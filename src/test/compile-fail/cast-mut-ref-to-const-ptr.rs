// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    unsafe {
        let my_num: &[i32; 2] = &[10, 20];
        let my_num: *mut i32 = my_num as *mut i32;
        //~ error: casting `&[i32; 2]` as `*mut i32` is invalid
        *my_num.offset(1) = 4;
        println!("{}", *my_num.offset(1));
    }
}
