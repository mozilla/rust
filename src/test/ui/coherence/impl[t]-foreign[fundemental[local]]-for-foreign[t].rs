#![feature(re_rebalance_coherence)]

// compile-flags:--crate-name=test
// aux-build:coherence_lib.rs
// check-pass

extern crate coherence_lib as lib;
use lib::*;
use std::rc::Rc;

struct Local;
struct Local1<S>(Rc<S>);

impl<T> Remote1<Box<Local>> for Rc<T> {}
impl<S, T> Remote1<Box<Local1<S>>> for Rc<T> {}

fn main() {}
