#![warn(unused)]
#[deny(warnings)]

const foo: isize = 3;
//~^ ERROR: should have an upper case name such as
//~^^ ERROR: constant item is never used

fn main() {}
