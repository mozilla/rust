#![feature(unsized_tuple_coercion, unsized_locals, unsized_fn_params)]
//~^ WARN the feature `unsized_locals` is incomplete and may not be safe to use and/or cause compiler crashes [incomplete_features]

struct A<X: ?Sized>(X);

fn udrop<T: ?Sized>(_x: T) {}
fn foo() -> Box<[u8]> {
    Box::new(*b"foo")
}
fn tfoo() -> Box<(i32, [u8])> {
    Box::new((42, *b"foo"))
}
fn afoo() -> Box<A<[u8]>> {
    Box::new(A(*b"foo"))
}

impl std::ops::Add<i32> for A<[u8]> {
    type Output = ();
    fn add(self, _rhs: i32) -> Self::Output {}
}

fn main() {
    udrop::<[u8]>(foo()[..]);
    //~^ERROR cannot move out of type `[u8]`, a non-copy slice
}
