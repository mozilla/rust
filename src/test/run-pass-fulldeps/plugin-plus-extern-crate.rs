// aux-build:macro_crate_test.rs
// ignore-stage1
// ignore-cross-compile
//
// macro_crate_test will not compile on a cross-compiled target because
// libsyntax is not compiled for it.

#![allow(plugin_as_library)]
#![feature(plugin)]
#![plugin(macro_crate_test)]

extern crate macro_crate_test;

fn main() {
    assert_eq!(1, make_a_1!());
    macro_crate_test::foo();
}
