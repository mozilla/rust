// run-pass
#![feature(const_generics)]
#![allow(incomplete_features)]

struct R;

impl R {
    fn method<const N: u8>(&self) -> u8 { N }
}
fn main() {
    assert_eq!(R.method::<1u8>(), 1);
}
