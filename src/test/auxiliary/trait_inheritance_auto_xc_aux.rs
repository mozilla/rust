// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub trait Foo { fn f(&self) -> int; }
pub trait Bar { fn g(&self) -> int; }
pub trait Baz { fn h(&self) -> int; }

pub trait Quux: Foo + Bar + Baz { }

impl<T:Foo + Bar + Baz> Quux for T { }
