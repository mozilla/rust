// Elided lifetimes within the type of a const parameters is disallowed. This matches the
// behaviour of trait bounds where `fn foo<T: Ord<&u8>>() {}` is illegal. Though we could change
// elided lifetimes within the type of a const parameters to be 'static, like elided
// lifetimes within const/static items.
// revisions: full min

#![cfg_attr(full, feature(const_generics))]
#![cfg_attr(full, allow(incomplete_features))]

struct A<const N: &u8>;
//~^ ERROR `&` without an explicit lifetime name cannot be used here
//[min]~^^ ERROR `&'static u8` is forbidden
trait B {}

impl<const N: &u8> A<N> {
//~^ ERROR `&` without an explicit lifetime name cannot be used here
//[min]~^^ ERROR `&'static u8` is forbidden
    fn foo<const M: &u8>(&self) {}
    //~^ ERROR `&` without an explicit lifetime name cannot be used here
    //[min]~^^ ERROR `&'static u8` is forbidden
}

impl<const N: &u8> B for A<N> {}
//~^ ERROR `&` without an explicit lifetime name cannot be used here
//[min]~^^ ERROR `&'static u8` is forbidden

fn bar<const N: &u8>() {}
//~^ ERROR `&` without an explicit lifetime name cannot be used here
//[min]~^^ ERROR `&'static u8` is forbidden

fn main() {}
