// run-rustfix
#![allow(dead_code)]
struct X<'a, T, 'b> { //~ ERROR incorrect parameter order
    x: &'a &'b T
}

fn main() {}
