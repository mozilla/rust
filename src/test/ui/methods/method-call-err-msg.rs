// Test that parameter cardinality or missing method error gets span exactly.

pub struct Foo;
impl Foo {
    fn zero(self) -> Foo { self }
    fn one(self, _: isize) -> Foo { self }
    fn two(self, _: isize, _: isize) -> Foo { self }
    fn three<T>(self, _: T, _: T, _: T) -> Foo { self }
}

fn main() {
    let x = Foo;
    x.zero(0)   //~ ERROR this function takes 0 arguments but 1 argument was supplied
     .one()     //~ ERROR this function takes 1 argument but 0 arguments were supplied
     .two(0);   //~ ERROR this function takes 2 arguments but 1 argument was supplied

    let y = Foo;
    y.zero()
     .take()    //~ ERROR no method named `take` found
     .one(0);
    y.three::<usize>(); //~ ERROR this function takes 3 arguments but 0 arguments were supplied
}
