// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name = "foo"]

// @has 'foo/index.html' '//h1' 'Crate foo'

// @has 'foo/foo_mod/index.html' '//h1' 'Module foo::foo_mod'
pub mod foo_mod {
    pub struct __Thing {}
}

extern {
    // @has 'foo/fn.foo_ffn.html' '//h1' 'Function foo::foo_ffn'
    pub fn foo_ffn();
}

// @has 'foo/fn.foo_fn.html' '//h1' 'Function foo::foo_fn'
pub fn foo_fn() {}

// @has 'foo/trait.FooTrait.html' '//h1' 'Trait foo::FooTrait'
pub trait FooTrait {}

// @has 'foo/struct.FooStruct.html' '//h1' 'Struct foo::FooStruct'
pub struct FooStruct;

// @has 'foo/enum.FooEnum.html' '//h1' 'Enum foo::FooEnum'
pub enum FooEnum {}

// @has 'foo/type.FooType.html' '//h1' 'Type Definition foo::FooType'
pub type FooType = FooStruct;

// @has 'foo/macro.foo_macro!.html' '//h1' 'Macro foo::foo_macro!'
#[macro_export]
macro_rules! foo_macro {
    () => ();
}

// @has 'foo/primitive.bool.html' '//h1' 'Primitive Type bool'
#[doc(primitive = "bool")]
mod bool {}

// @has 'foo/static.FOO_STATIC.html' '//h1' 'Static foo::FOO_STATIC'
pub static FOO_STATIC: FooStruct = FooStruct;

extern {
    // @has 'foo/static.FOO_FSTATIC.html' '//h1' 'Static foo::FOO_FSTATIC'
    pub static FOO_FSTATIC: FooStruct;
}

// @has 'foo/constant.FOO_CONSTANT.html' '//h1' 'Constant foo::FOO_CONSTANT'
pub const FOO_CONSTANT: FooStruct = FooStruct;
