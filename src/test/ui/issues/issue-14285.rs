trait Foo {
    fn dummy(&self) { }
}

struct A;

impl Foo for A {}

struct B<'a>(&'a (Foo+'a));

fn foo<'a>(a: &Foo) -> B<'a> {
    B(a)    //~ ERROR 22:5: 22:9: explicit lifetime required in the type of `a` [E0621]
}

fn main() {
    let _test = foo(&A);
}
