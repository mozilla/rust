// compile-flags: --edition 2018

#![feature(try_blocks)]

fn main() {
    while try { false } {}
    //~^ ERROR the trait bound `bool: Bubble` is not satisfied
}
