struct Foo {
    a: u8,
    b: (),
    c: &'static str,
    d: Option<isize>,
}

// EMIT_MIR flatten_locals.main.FlattenLocals.diff
fn main() {
    let Foo { a, b, c, d } = Foo { a: 5, b: (), c: "a", d: Some(-4) };
    let _ = a;
    let _ = b;
    let _ = c;
    let _ = d;
}
