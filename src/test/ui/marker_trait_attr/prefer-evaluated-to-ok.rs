// check-pass
// Regression test for #84917.
#![feature(marker_trait_attr)]

#[marker]
pub trait F {}
impl<T> F for T where T: Copy {}
impl<T> F for T where T: 'static {}

fn main() {}
