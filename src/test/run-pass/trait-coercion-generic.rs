trait Trait<T> {
    fn f(&self, x: T);
}

#[derive(Copy, Clone)]
struct Struct {
    x: isize,
    y: isize,
}

impl Trait<&'static str> for Struct {
    fn f(&self, x: &'static str) {
        println!("Hi, {}!", x);
    }
}

pub fn main() {
    let a = Struct { x: 1, y: 2 };
    let b: Box<Trait<&'static str>> = Box::new(a);
    b.f("Mary");
    let c: &Trait<&'static str> = &a;
    c.f("Joe");
}
