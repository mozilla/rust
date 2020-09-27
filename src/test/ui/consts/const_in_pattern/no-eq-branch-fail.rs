#![warn(indirect_structural_match)]

struct NoEq;

enum Foo {
    Bar,
    Baz,
    Qux(NoEq),
}

// Even though any of these values can be compared structurally, we still disallow it in a pattern
// because `Foo` does not impl `PartialEq`.
const BAR_BAZ: Foo = if 42 == 42 {
    Foo::Baz
} else {
    Foo::Bar
};

fn main() {
    match Foo::Qux(NoEq) {
        BAR_BAZ => panic!(),
        //~^ ERROR must be annotated with `#[derive(PartialEq, Eq)]`
        _ => {}
    }
}
