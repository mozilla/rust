// Tests that even when a type parameter doesn't implement a required
// super-builtin-kind of a trait, if the type parameter is never used,
// the type can implement the trait anyway.

// pretty-expanded FIXME #23616

use std::marker;

trait Foo : Send { }

struct X<T> { marker: marker::PhantomData<T> }

impl<T:Send> Foo for X<T> { }

pub fn main() { }
