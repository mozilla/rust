// run-pass
// pretty-expanded FIXME(#23616):

#![allow(unused_variables)]

pub fn main() {
    // We should be able to type infer inside of ||s.
    let _f = || {
        let i = 10;
    };
}
