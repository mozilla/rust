// run-pass
// pretty-expanded FIXME(#23616)

fn f<T:PartialEq + PartialOrd>(_: T) {
}

pub fn main() {
    f(3);
}
