#![feature(box_syntax, box_patterns)]

struct Foo{}

pub fn main() {
    let b = box Foo{};
    let box f = &b;
    let _: &Foo = f;

    match &&&b {
        box f => {
            let _: &Foo = f;
        },
        _ => panic!(),
    }
}
