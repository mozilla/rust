// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


fn is_sized<T:Sized>() { }
fn not_sized<T: ?Sized>() { }

enum Foo<U> { FooSome(U), FooNone }
fn foo1<T>() { not_sized::<Foo<T>>() } // Hunky dory.
fn foo2<T: ?Sized>() { not_sized::<Foo<T>>() }
//~^ ERROR the trait `core::marker::Sized` is not implemented
//
// Not OK: `T` is not sized.

enum Bar<U: ?Sized> { BarSome(U), BarNone }
fn bar1<T: ?Sized>() { not_sized::<Bar<T>>() }
fn bar2<T: ?Sized>() { is_sized::<Bar<T>>() }
//~^ ERROR the trait `core::marker::Sized` is not implemented
//
// Not OK: `Bar<T>` is not sized, but it should be.

fn main() { }
