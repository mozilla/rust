// run-pass
#![allow(dead_code)]
// Regression test for issue #7740

// pretty-expanded FIXME(#23616)

pub fn main() {
    static A: &'static char = &'A';
}
