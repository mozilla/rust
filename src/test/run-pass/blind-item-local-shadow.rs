mod bar {
    pub fn foo() -> bool { true }
}

fn main() {
    let foo = || false;
    use bar::foo;
    assert_eq!(foo(), false);
}
