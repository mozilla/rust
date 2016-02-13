// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:rustdoc-extern-method.rs
// ignore-cross-compile
// check-search-index

#![feature(unboxed_closures)]

extern crate rustdoc_extern_method as foo;

// @has extern_method/trait.Foo.html //pre "pub trait Foo"
// @has - '//*[@id="tymethod.foo"]//code' 'extern "rust-call" fn foo'
// @has - '//*[@id="method.foo_"]//code' 'extern "rust-call" fn foo_'
pub use foo::Foo;

// @has extern_method/trait.Bar.html //pre "pub trait Bar"
pub trait Bar {
    // @has - '//*[@id="tymethod.bar"]//code' 'extern "rust-call" fn bar'
    extern "rust-call" fn bar(&self, _: ());
    // @has - '//*[@id="method.bar_"]//code' 'extern "rust-call" fn bar_'
    extern "rust-call" fn bar_(&self, _: ()) { }
}

/* !search-index
{
    "extern_method": {
        "extern_method::Bar": [
            "Trait"
        ],
        "extern_method::Bar<Trait>::bar": [
            "TyMethod()"
        ],
        "extern_method::Bar<Trait>::bar_": [
            "Method()"
        ],
        "extern_method::Foo": [
            "Trait"
        ],
        "extern_method::Foo<Trait>::foo": [
            "TyMethod()"
        ],
        "extern_method::Foo<Trait>::foo_": [
            "Method()"
        ]
    }
}
*/
