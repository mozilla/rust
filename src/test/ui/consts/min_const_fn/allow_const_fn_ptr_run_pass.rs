// run-pass
#![feature(allow_internal_unstable)]
#![feature(const_fn_fn_ptr_basics)]

#![feature(rustc_attrs, staged_api)]
#![stable(feature = "rust1", since = "1.0.0")]

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(since="1.0.0", feature = "mep")]
#[allow_internal_unstable(const_fn_fn_ptr_basics)]
const fn takes_fn_ptr(_: fn()) {}

const FN: fn() = || ();

const fn gives_fn_ptr() {
    takes_fn_ptr(FN)
}

fn main() {
    gives_fn_ptr();
}
