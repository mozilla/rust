// revisions: min_tait full_tait
#![feature(min_type_alias_impl_trait)]
#![cfg_attr(full_tait, feature(type_alias_impl_trait))]
//[full_tait]~^ WARN incomplete

use std::fmt::Debug;

fn main() {}

type Two<T, U> = impl Debug;

fn two<T: Debug>(t: T) -> Two<T, u32> {
    //~^ ERROR non-defining opaque type use in defining scope
    (t, 4i8)
}

fn three<T: Debug, U>(t: T) -> Two<T, U> {
    (t, 5i8)
}

trait Bar {
    type Blub: Debug;
    const FOO: Self::Blub;
}

impl Bar for u32 {
    type Blub = i32;
    const FOO: i32 = 42;
}

fn four<T: Debug, U: Bar>(t: T) -> Two<T, U> {
    (t, <U as Bar>::FOO)
}

fn is_sync<T: Sync>() {}

fn asdfl() {
    //FIXME(oli-obk): these currently cause cycle errors
    //is_sync::<Two<i32, u32>>();
    //is_sync::<Two<i32, *const i32>>();
}
