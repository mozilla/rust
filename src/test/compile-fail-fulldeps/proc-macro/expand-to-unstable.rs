// aux-build:derive-unstable.rs
// ignore-stage1

#![allow(warnings)]

#[macro_use]
extern crate derive_unstable;

#[derive(Unstable)]
//~^ ERROR: use of unstable library feature
struct A;

fn main() {
    unsafe { foo(); }
}
