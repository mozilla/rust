#![crate_name = "foo"]

#![feature(optin_builtin_traits)]

pub struct Foo;

// @has foo/struct.Foo.html
// @!has - '//*[@class="synthetic-implementations"]' 'Auto Trait Implementations'
impl !Send for Foo {}
impl !Sync for Foo {}
