// run-rustfix
#![allow(dead_code, unused_variables)]
fn foo<'a, T, 'b>(x: &'a T) {} //~ ERROR incorrect parameter order

fn main() {}
