// @has 'glob_shadowing/index.html'
// @count - '//tr[@class="module-item"]' 5
// @!has - '//tr[@class="module-item"]' 'sub1::describe'
// @has - '//tr[@class="module-item"]' 'sub2::describe'

// @!has - '//tr[@class="module-item"]' 'sub1::describe2'

// @!has - '//tr[@class="module-item"]' 'sub1::prelude'
// @has - '//tr[@class="module-item"]' 'mod::prelude'

// @has - '//tr[@class="module-item"]' 'sub1::Foo (struct)'
// @has - '//tr[@class="module-item"]' 'mod::Foo (function)'

// @has - '//tr[@class="module-item"]' 'sub4::inner::X'

// @has 'glob_shadowing/fn.describe.html'
// @has - '//div[@class="docblock"]' 'sub2::describe'

mod sub1 {
    // this should be shadowed by sub2::describe
    /// sub1::describe
    pub fn describe() -> &'static str {
        "sub1::describe"
    }

    // this should be shadowed by mod::prelude
    /// sub1::prelude
    pub mod prelude {
    }

    // this should *not* be shadowed, because sub1::Foo and mod::Foo are in different namespaces
    /// sub1::Foo (struct)
    pub struct Foo;

    // this should be shadowed, 
    // because both sub1::describe2 and sub3::describe2 are from glob reexport
    /// sub1::describe2
    pub fn describe2() -> &'static str {
        "sub1::describe2"
    }
}

mod sub2 {
    /// sub2::describe
    pub fn describe() -> &'static str {
        "sub2::describe"
    }
}

mod sub3 {
    // this should be shadowed
    // because both sub1::describe2 and sub3::describe2 are from glob reexport
    /// sub3::describe2
    pub fn describe2() -> &'static str {
        "sub3::describe2"
    }
}

mod sub4 {
    // this should be shadowed by sub4::inner::X
    /// sub4::X
    pub const X: usize = 0;
    pub mod inner {
        pub use super::*;
        /// sub4::inner::X
        pub const X: usize = 1;
    }
}

/// mod::Foo (function)
pub fn Foo() {}

#[doc(inline)]
pub use sub2::describe;

#[doc(inline)]
pub use sub1::*;

#[doc(inline)]
pub use sub3::*;

#[doc(inline)]
pub use sub4::inner::*;

/// mod::prelude
pub mod prelude {}