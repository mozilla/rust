// run-rustfix
#![allow(dead_code)]

macro_rules! y {
    () => {}
}

mod m {
    pub const A: i32 = 0;
}

mod foo {
    // FIXME: UsePlacementFinder is broken because active attributes are
    // removed, and thus the `derive` attribute here is not in the AST.
    // An inert attribute should work, though.
    // #[derive(Debug)]
    #[allow(warnings)]
    pub struct Foo;

    // test whether the use suggestion isn't
    // placed into the expansion of `#[derive(Debug)]
    type Bar = Path; //~ ERROR cannot find
}

fn main() {
    y!();
    let _ = A; //~ ERROR cannot find
    foo();
}

fn foo() {
    type Dict<K, V> = HashMap<K, V>; //~ ERROR cannot find
}
