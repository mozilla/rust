#![feature(const_trait_impl)]
#![allow(incomplete_features)]

#![feature(staged_api)]
#![stable(feature = "rust1", since = "1.0.0")]

#[stable(feature = "rust1", since = "1.0.0")]
pub trait MyTrait {
    #[stable(feature = "rust1", since = "1.0.0")]
    fn func();
}

#[stable(feature = "rust1", since = "1.0.0")]
pub struct Unstable;

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "staged", issue = "none")]
impl const MyTrait for Unstable {
    fn func() {

    }
}
