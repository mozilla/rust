//! Documenting the kinds of lints emitted by rustdoc.
//!
//! ```
//! println!("sup");
//! ```

#![deny(rustdoc)]

/// what up, let's make an [error]
///
/// ```
/// println!("sup");
/// ```
pub fn link_error() {} //~^^^^^ ERROR unresolved link to `error`

/// wait, this doesn't have a doctest?
pub fn no_doctest() {} //~^ ERROR missing code example in this documentation

/// wait, this *does* have a doctest?
///
/// ```
/// println!("sup");
/// ```
fn private_doctest() {} //~^^^^^ ERROR documentation test in private item

pub fn no_doc() {}
//~^ ERROR missing documentation for a function
//~^^ ERROR missing code example in this documentation
