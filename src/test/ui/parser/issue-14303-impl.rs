// run-rustfix
#![allow(dead_code)]
struct X<T>(T);

impl<'a, T, 'b> X<T> {} //~ ERROR incorrect parameter order

fn main() {}
