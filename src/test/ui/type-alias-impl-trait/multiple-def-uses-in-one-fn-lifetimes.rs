#![feature(min_type_alias_impl_trait)]

type X<'a, 'b> = impl std::fmt::Debug;

fn f<'t, 'u>(a: &'t u32, b: &'u u32) -> (X<'t, 'u>, X<'u, 't>) {
    (a, a)
}

fn main() {}
