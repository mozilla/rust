// Regression test for #68642

#![feature(generic_associated_types)]
//~^ WARNING the feature `generic_associated_types` is incomplete and may not

trait Fun {
    type F<'a>: Fn() -> u32;

    fn callme<'a>(f: Self::F<'a>) -> u32 {
        f()
    }
}

impl<T> Fun for T {
    type F<'a> = Self;
    //~^ ERROR expected a `Fn<()>` closure, found `T`
}

fn main() {
    <fn() -> usize>::callme(|| 1);
}
