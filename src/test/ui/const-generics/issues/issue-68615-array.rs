// [full] check-pass
// revisions: full min
#![cfg_attr(full, feature(const_generics))]
#![cfg_attr(full, allow(incomplete_features))]

struct Foo<const V: [usize; 0] > {}
//[min]~^ ERROR `[usize; 0]` is forbidden as the type of a const parameter

type MyFoo = Foo<{ [] }>;

fn main() {
    let _ = Foo::<{ [] }> {};
}
