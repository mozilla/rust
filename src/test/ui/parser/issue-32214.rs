// run-rustfix
pub trait Trait<T> { type Item; }

pub fn test<W, I: Trait<Item=(), W> >() {}
//~^ ERROR incorrect parameter order

fn main() { }
