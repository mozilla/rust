// aux-build:coherence_lib.rs

// pretty-expanded FIXME #23616

extern crate coherence_lib as lib;
use lib::Remote1;

struct Foo<T>(T);

impl<T> Remote1<T> for Foo<T> { }

fn main() { }
