// only-i686-pc-windows-msvc
// compile-flags: --crate-type lib --emit link
#![allow(clashing_extern_declarations)]
#![feature(raw_dylib)]
//~^ WARN the feature `raw_dylib` is incomplete
#[link(name = "foo", kind = "raw-dylib")]
extern "C" {
    fn f(x: i32);
}

pub fn lib_main() {
    #[link(name = "foo", kind = "raw-dylib")]
    extern "stdcall" {
        fn f(x: i32);
        //~^ ERROR multiple definitions of external function `f` from library `foo.dll` have different calling conventions
    }

    unsafe { f(42); }
}
