// This test checks that it proper item type will be suggested when
// using the `_` type placeholder.

fn test1() -> _ { Some(42) }
//~^ ERROR the type placeholder `_` is not allowed within types on item signatures for return types

const TEST2: _ = 42u32;
//~^ ERROR the type placeholder `_` is not allowed within types on item signatures for constants

const TEST3: _ = Some(42);
//~^ ERROR the type placeholder `_` is not allowed within types on item signatures for constants

const TEST4: fn() -> _ = 42;
//~^ ERROR the type placeholder `_` is not allowed within types on item signatures for functions

trait Test5 {
    const TEST5: _ = 42;
    //~^ ERROR the type placeholder `_` is not allowed within types on item signatures for constants
}

struct Test6;

impl Test6 {
    const TEST6: _ = 13;
    //~^ ERROR the type placeholder `_` is not allowed within types on item signatures for constants
}

pub fn main() {
    let _: Option<usize> = test1();
    let _: f64 = test1();
    let _: Option<i32> = test1();
}
