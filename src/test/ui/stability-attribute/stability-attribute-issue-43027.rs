#![feature(staged_api)]
#![stable(feature = "test", since = "0")]

#[stable(feature = "test", since = "0")]
pub struct Reverse<T>(pub T); //~ ERROR This node does not have a stability attribute

fn main() {
    // Make sure the field is used to fill the stability cache
    Reverse(0).0;
}
