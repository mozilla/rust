// Regression test for #47470: cached results of projections were
// causing region relations not to be enforced at all the places where
// they have to be enforced.

#![feature(nll)]

struct Foo<'a>(&'a ());
trait Bar {
    type Assoc;
    fn get(self) -> Self::Assoc;
}

impl<'a> Bar for Foo<'a> {
    type Assoc = &'a u32;
    fn get(self) -> Self::Assoc {
        let local = 42;
        &local //~ ERROR `local` does not live long enough
    }
}

fn main() {
    let f = Foo(&()).get();
    println!("{}", f);
}
