//https://github.com/rust-lang/rust/issues/31364

#![feature(const_fn)]
const fn a() -> usize { b() }
const fn b() -> usize { a() }
const ARR: [i32; a()] = [5; 6]; //~ ERROR could not evaluate constant expression

fn main(){}
