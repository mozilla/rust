#[derive(Default)] //~ ERROR deriving `Default` on enums is experimental
enum Foo {
    #[default] //~ ERROR `#[default]` enum variants are experimental
    Alpha,
}

fn main() {}
