// compile-flags: --playground-url="" -Z unstable-options

#![crate_name = "foo"]

//! module docs
//!
//! ```
//! println!("Hello, world!");
//! ```

// @!has foo/index.html '//a[@class="test-arrow"]' "Run"
