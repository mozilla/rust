// FIXME: missing sysroot spans (#53081)
// ignore-i586-unknown-linux-gnu
// ignore-i586-unknown-linux-musl
// ignore-i686-unknown-linux-musl

// ignore-wasm32-bare no libc to test ffi with

#![feature(rustc_private)]

extern crate libc;

fn main() {
    let x : *const Vec<isize> = &vec![1,2,3];
    let y : *const libc::c_void = x as *const libc::c_void;
    unsafe {
        let _z = (*y).clone();
        //~^ ERROR no method named `clone` found
    }
}
