// Test that the NLL `relate_tys` code correctly deduces that a
// function returning either argument CANNOT be upcast to one
// that returns always its first argument.
//
// compile-flags:-Zno-leak-check

#![feature(nll)]

fn make_it() -> for<'a> fn(&'a u32, &'a u32) -> &'a u32 {
    panic!()
}

fn main() {
    let a: for<'a, 'b> fn(&'a u32, &'b u32) -> &'a u32 = make_it();
    //~^ ERROR higher-ranked subtype error
    drop(a);
}
