// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub mod foo {
    pub struct Foo;
}

// @has please_inline/a/index.html
pub mod a {
    // @!has - 'pub use foo::'
    // @has please_inline/a/Foo.t.html
    #[doc(inline)]
    pub use foo::Foo;
}

// @has please_inline/b/index.html
pub mod b {
    // @has - 'pub use foo::'
    // @!has please_inline/b/Foo.t.html
    #[feature(inline)]
    pub use foo::Foo;
}
