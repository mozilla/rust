// check-pass

#![feature(const_generics)]
//~^ WARN the feature `const_generics` is incomplete and may cause the compiler to crash

use std::mem;

fn foo<const SIZE: usize>() {
    let arr: [u8; SIZE] = unsafe {

        let array: [u8; SIZE] = mem::zeroed();
        array
    };
}

fn main() {}
