// run-rustfix
#![allow(dead_code)]
enum X<'a, T, 'b> { //~ ERROR incorrect parameter order
    A(&'a &'b T)
}

fn main() {}
