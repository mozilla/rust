// #[deprecated] can't be used in staged api

#![feature(staged_api)]

#![stable(feature = "stable_test_feature", since = "1.0.0")]

#[deprecated]
fn main() { } //~ERROR `#[deprecated]` cannot be used in staged api
