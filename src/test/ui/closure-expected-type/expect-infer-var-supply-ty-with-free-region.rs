// compile-pass

fn with_closure<F, A>(_: F)
    where F: FnOnce(A, &u32)
{
}

fn foo() {
    // This version works; we infer `A` to be `u32`, and take the type
    // of `y` to be `&u32`.
    with_closure(|x: u32, y| {});
}

fn bar<'x>(x: &'x u32) {
    // Same.
    with_closure(|x: &'x u32, y| {});
}

fn main() { }
