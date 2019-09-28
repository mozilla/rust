#![feature(re_rebalance_coherence)]

// compile-flags:--crate-name=test
// aux-build:coherence_lib.rs

extern crate coherence_lib as lib;
use lib::*;
use std::rc::Rc;

struct Local;

impl<T> Remote1<T> for Box<T> {
    //~^ ERROR type parameter `T` must be used as the type parameter for some local type
}

impl<'a, A, B> Remote1<A> for &'a B {
    //~^ ERROR type parameter `B` must be used as the type parameter for some local type
}

fn main() {}
