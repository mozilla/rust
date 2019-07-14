// Test basic or-patterns when the target pattern type will be lowered to a
// `SwitchInt` (an `enum`).
// run-pass
#![feature(or_patterns)]
//~^ WARN the feature `or_patterns` is incomplete and may cause the compiler to crash

#[derive(Debug)]
enum Test {
    Foo,
    Bar,
    Baz,
    Qux
}

fn test(x: Option<Test>) -> bool {
    match x {
        // most simple case
        Some(Test::Bar | Test::Qux) => true,
        // wild case
        Some(_) => false,
        // empty case
        None => false,
    }
}

fn main() {
    assert!(!test(Some(Test::Foo)));
    assert!(test(Some(Test::Bar)));
    assert!(!test(Some(Test::Baz)));
    assert!(test(Some(Test::Qux)));
    assert!(!test(None))
}
