use std::marker::PhantomData;

struct B<T, const N: T>(PhantomData<[T; N]>); //~ ERROR const generics are unstable
//~^ ERROR the type of const parameters must not depend on other generic parameters

fn main() {}
