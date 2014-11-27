// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!

Cross-platform path support

This module implements support for two flavors of paths. `PosixPath` represents
a path on any unix-like system, whereas `WindowsPath` represents a path on
Windows. This module also exposes a typedef `Path` which is equal to the
appropriate platform-specific path variant.

Both `PosixPath` and `WindowsPath` implement a trait `GenericPath`, which
contains the set of methods that behave the same for both paths. They each also
implement some methods that could not be expressed in `GenericPath`, yet behave
identically for both path flavors, such as `.components()`.

The three main design goals of this module are 1) to avoid unnecessary
allocation, 2) to behave the same regardless of which flavor of path is being
used, and 3) to support paths that cannot be represented in UTF-8 (as Linux has
no restriction on paths beyond disallowing NUL).

## Usage

Usage of this module is fairly straightforward. Unless writing platform-specific
code, `Path` should be used to refer to the platform-native path.

Creation of a path is typically done with either `Path::new(some_str)` or
`Path::new(some_vec)`. This path can be modified with `.push()` and
`.pop()` (and other setters). The resulting Path can either be passed to another
API that expects a path, or can be turned into a `&[u8]` with `.as_vec()` or a
`Option<&str>` with `.as_str()`. Similarly, attributes of the path can be queried
with methods such as `.filename()`. There are also methods that return a new
path instead of modifying the receiver, such as `.join()` or `.dir_path()`.

Paths are always kept in normalized form. This means that creating the path
`Path::new("a/b/../c")` will return the path `a/c`. Similarly any attempt
to mutate the path will always leave it in normalized form.

When rendering a path to some form of output, there is a method `.display()`
which is compatible with the `format!()` parameter `{}`. This will render the
path as a string, replacing all non-utf8 sequences with the Replacement
Character (U+FFFD). As such it is not suitable for passing to any API that
actually operates on the path; it is only intended for display.

## Example

```rust
use std::io::fs::PathExtensions;

let mut path = Path::new("/tmp/path");
println!("path: {}", path.display());
path.set_filename("foo");
path.push("bar");
println!("new path: {}", path.display());
println!("path exists: {}", path.exists());
```

*/

#![experimental]

use core::kinds::Sized;
use c_str::CString;
use clone::Clone;
use fmt;
use iter::Iterator;
use option::{Option, None, Some};
use str;
use str::{MaybeOwned, Str, StrPrelude};
use string::String;
use slice::{AsSlice, CloneSliceAllocPrelude};
use slice::{PartialEqSlicePrelude, SlicePrelude};
use vec::Vec;

/// Typedef for POSIX file paths.
/// See `posix::Path` for more info.
pub use self::posix::Path as PosixPath;

/// Typedef for Windows file paths.
/// See `windows::Path` for more info.
pub use self::windows::Path as WindowsPath;

/// Typedef for the platform-native path type
#[cfg(unix)]
pub use self::posix::Path as Path;
/// Typedef for the platform-native path type
#[cfg(windows)]
pub use self::windows::Path as Path;

/// Typedef for the platform-native component iterator
#[cfg(unix)]
pub use self::posix::Components as Components;
/// Typedef for the platform-native component iterator
#[cfg(windows)]
pub use self::windows::Components as Components;

/// Typedef for the platform-native str component iterator
#[cfg(unix)]
pub use self::posix::StrComponents as StrComponents;
/// Typedef for the platform-native str component iterator
#[cfg(windows)]
pub use self::windows::StrComponents as StrComponents;

/// Alias for the platform-native separator character.
#[cfg(unix)]
pub use self::posix::SEP as SEP;
/// Alias for the platform-native separator character.
#[cfg(windows)]
pub use self::windows::SEP as SEP;

/// Alias for the platform-native separator byte.
#[cfg(unix)]
pub use self::posix::SEP_BYTE as SEP_BYTE;
/// Alias for the platform-native separator byte.
#[cfg(windows)]
pub use self::windows::SEP_BYTE as SEP_BYTE;

/// Typedef for the platform-native separator char func
#[cfg(unix)]
pub use self::posix::is_sep as is_sep;
/// Typedef for the platform-native separator char func
#[cfg(windows)]
pub use self::windows::is_sep as is_sep;
/// Typedef for the platform-native separator byte func
#[cfg(unix)]
pub use self::posix::is_sep_byte as is_sep_byte;
/// Typedef for the platform-native separator byte func
#[cfg(windows)]
pub use self::windows::is_sep_byte as is_sep_byte;

pub mod posix;
pub mod windows;

/// A trait that represents the generic operations available on paths
pub trait GenericPath: Clone + GenericPathUnsafe {
    /// Creates a new Path from a byte vector or string.
    /// The resulting Path will always be normalized.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let path = Path::new("foo/bar");
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics the task if the path contains a NUL.
    ///
    /// See individual Path impls for additional restrictions.
    #[inline]
    fn new<T: BytesContainer>(path: T) -> Self { unimplemented!() }

    /// Creates a new Path from a byte vector or string, if possible.
    /// The resulting Path will always be normalized.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let x: &[u8] = b"foo\0";
    /// assert!(Path::new_opt(x).is_none());
    /// # }
    /// ```
    #[inline]
    fn new_opt<T: BytesContainer>(path: T) -> Option<Self> { unimplemented!() }

    /// Returns the path as a string, if possible.
    /// If the path is not representable in utf-8, this returns None.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("/abc/def");
    /// assert_eq!(p.as_str(), Some("/abc/def"));
    /// # }
    /// ```
    #[inline]
    fn as_str<'a>(&'a self) -> Option<&'a str> { unimplemented!() }

    /// Returns the path as a byte vector
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def");
    /// assert_eq!(p.as_vec(), b"abc/def");
    /// # }
    /// ```
    fn as_vec<'a>(&'a self) -> &'a [u8];

    /// Converts the Path into an owned byte vector
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def");
    /// assert_eq!(p.into_vec(), b"abc/def".to_vec());
    /// // attempting to use p now results in "error: use of moved value"
    /// # }
    /// ```
    fn into_vec(self) -> Vec<u8>;

    /// Returns an object that implements `Show` for printing paths
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def");
    /// println!("{}", p.display()); // prints "abc/def"
    /// # }
    /// ```
    fn display<'a>(&'a self) -> Display<'a, Self> { unimplemented!() }

    /// Returns an object that implements `Show` for printing filenames
    ///
    /// If there is no filename, nothing will be printed.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def");
    /// println!("{}", p.filename_display()); // prints "def"
    /// # }
    /// ```
    fn filename_display<'a>(&'a self) -> Display<'a, Self> { unimplemented!() }

    /// Returns the directory component of `self`, as a byte vector (with no trailing separator).
    /// If `self` has no directory component, returns ['.'].
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def/ghi");
    /// assert_eq!(p.dirname(), b"abc/def");
    /// # }
    /// ```
    fn dirname<'a>(&'a self) -> &'a [u8];

    /// Returns the directory component of `self`, as a string, if possible.
    /// See `dirname` for details.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def/ghi");
    /// assert_eq!(p.dirname_str(), Some("abc/def"));
    /// # }
    /// ```
    #[inline]
    fn dirname_str<'a>(&'a self) -> Option<&'a str> { unimplemented!() }

    /// Returns the file component of `self`, as a byte vector.
    /// If `self` represents the root of the file hierarchy, returns None.
    /// If `self` is "." or "..", returns None.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def/ghi");
    /// assert_eq!(p.filename(), Some(b"ghi"));
    /// # }
    /// ```
    fn filename<'a>(&'a self) -> Option<&'a [u8]>;

    /// Returns the file component of `self`, as a string, if possible.
    /// See `filename` for details.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def/ghi");
    /// assert_eq!(p.filename_str(), Some("ghi"));
    /// # }
    /// ```
    #[inline]
    fn filename_str<'a>(&'a self) -> Option<&'a str> { unimplemented!() }

    /// Returns the stem of the filename of `self`, as a byte vector.
    /// The stem is the portion of the filename just before the last '.'.
    /// If there is no '.', the entire filename is returned.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("/abc/def.txt");
    /// assert_eq!(p.filestem(), Some(b"def"));
    /// # }
    /// ```
    fn filestem<'a>(&'a self) -> Option<&'a [u8]> { unimplemented!() }

    /// Returns the stem of the filename of `self`, as a string, if possible.
    /// See `filestem` for details.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("/abc/def.txt");
    /// assert_eq!(p.filestem_str(), Some("def"));
    /// # }
    /// ```
    #[inline]
    fn filestem_str<'a>(&'a self) -> Option<&'a str> { unimplemented!() }

    /// Returns the extension of the filename of `self`, as an optional byte vector.
    /// The extension is the portion of the filename just after the last '.'.
    /// If there is no extension, None is returned.
    /// If the filename ends in '.', the empty vector is returned.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def.txt");
    /// assert_eq!(p.extension(), Some(b"txt"));
    /// # }
    /// ```
    fn extension<'a>(&'a self) -> Option<&'a [u8]> { unimplemented!() }

    /// Returns the extension of the filename of `self`, as a string, if possible.
    /// See `extension` for details.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def.txt");
    /// assert_eq!(p.extension_str(), Some("txt"));
    /// # }
    /// ```
    #[inline]
    fn extension_str<'a>(&'a self) -> Option<&'a str> { unimplemented!() }

    /// Replaces the filename portion of the path with the given byte vector or string.
    /// If the replacement name is [], this is equivalent to popping the path.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let mut p = Path::new("abc/def.txt");
    /// p.set_filename("foo.dat");
    /// assert!(p == Path::new("abc/foo.dat"));
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics the task if the filename contains a NUL.
    #[inline]
    fn set_filename<T: BytesContainer>(&mut self, filename: T) { unimplemented!() }

    /// Replaces the extension with the given byte vector or string.
    /// If there is no extension in `self`, this adds one.
    /// If the argument is [] or "", this removes the extension.
    /// If `self` has no filename, this is a no-op.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let mut p = Path::new("abc/def.txt");
    /// p.set_extension("csv");
    /// assert!(p == Path::new("abc/def.csv"));
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics the task if the extension contains a NUL.
    fn set_extension<T: BytesContainer>(&mut self, extension: T) { unimplemented!() }

    /// Returns a new Path constructed by replacing the filename with the given
    /// byte vector or string.
    /// See `set_filename` for details.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let mut p = Path::new("abc/def.txt");
    /// assert!(p.with_filename("foo.dat") == Path::new("abc/foo.dat"));
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics the task if the filename contains a NUL.
    #[inline]
    fn with_filename<T: BytesContainer>(&self, filename: T) -> Self { unimplemented!() }

    /// Returns a new Path constructed by setting the extension to the given
    /// byte vector or string.
    /// See `set_extension` for details.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let mut p = Path::new("abc/def.txt");
    /// assert!(p.with_extension("csv") == Path::new("abc/def.csv"));
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics the task if the extension contains a NUL.
    #[inline]
    fn with_extension<T: BytesContainer>(&self, extension: T) -> Self { unimplemented!() }

    /// Returns the directory component of `self`, as a Path.
    /// If `self` represents the root of the filesystem hierarchy, returns `self`.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def/ghi");
    /// assert!(p.dir_path() == Path::new("abc/def"));
    /// # }
    /// ```
    fn dir_path(&self) -> Self { unimplemented!() }

    /// Returns a Path that represents the filesystem root that `self` is rooted in.
    ///
    /// If `self` is not absolute, or vol/cwd-relative in the case of Windows, this returns None.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// assert!(Path::new("abc/def").root_path() == None);
    /// assert!(Path::new("/abc/def").root_path() == Some(Path::new("/")));
    /// # }
    /// ```
    fn root_path(&self) -> Option<Self>;

    /// Pushes a path (as a byte vector or string) onto `self`.
    /// If the argument represents an absolute path, it replaces `self`.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let mut p = Path::new("foo/bar");
    /// p.push("baz.txt");
    /// assert!(p == Path::new("foo/bar/baz.txt"));
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics the task if the path contains a NUL.
    #[inline]
    fn push<T: BytesContainer>(&mut self, path: T) { unimplemented!() }

    /// Pushes multiple paths (as byte vectors or strings) onto `self`.
    /// See `push` for details.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let mut p = Path::new("foo");
    /// p.push_many(&["bar", "baz.txt"]);
    /// assert!(p == Path::new("foo/bar/baz.txt"));
    /// # }
    /// ```
    #[inline]
    fn push_many<T: BytesContainer>(&mut self, paths: &[T]) { unimplemented!() }

    /// Removes the last path component from the receiver.
    /// Returns `true` if the receiver was modified, or `false` if it already
    /// represented the root of the file hierarchy.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let mut p = Path::new("foo/bar/baz.txt");
    /// p.pop();
    /// assert!(p == Path::new("foo/bar"));
    /// # }
    /// ```
    fn pop(&mut self) -> bool;

    /// Returns a new Path constructed by joining `self` with the given path
    /// (as a byte vector or string).
    /// If the given path is absolute, the new Path will represent just that.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("/foo");
    /// assert!(p.join("bar.txt") == Path::new("/foo/bar.txt"));
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics the task if the path contains a NUL.
    #[inline]
    fn join<T: BytesContainer>(&self, path: T) -> Self { unimplemented!() }

    /// Returns a new Path constructed by joining `self` with the given paths
    /// (as byte vectors or strings).
    /// See `join` for details.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("foo");
    /// let fbbq = Path::new("foo/bar/baz/quux.txt");
    /// assert!(p.join_many(&["bar", "baz", "quux.txt"]) == fbbq);
    /// # }
    /// ```
    #[inline]
    fn join_many<T: BytesContainer>(&self, paths: &[T]) -> Self { unimplemented!() }

    /// Returns whether `self` represents an absolute path.
    /// An absolute path is defined as one that, when joined to another path, will
    /// yield back the same absolute path.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("/abc/def");
    /// assert!(p.is_absolute());
    /// # }
    /// ```
    fn is_absolute(&self) -> bool;

    /// Returns whether `self` represents a relative path.
    /// Typically this is the inverse of `is_absolute`.
    /// But for Windows paths, it also means the path is not volume-relative or
    /// relative to the current working directory.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("abc/def");
    /// assert!(p.is_relative());
    /// # }
    /// ```
    fn is_relative(&self) -> bool { unimplemented!() }

    /// Returns whether `self` is equal to, or is an ancestor of, the given path.
    /// If both paths are relative, they are compared as though they are relative
    /// to the same parent path.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("foo/bar/baz/quux.txt");
    /// let fb = Path::new("foo/bar");
    /// let bq = Path::new("baz/quux.txt");
    /// assert!(fb.is_ancestor_of(&p));
    /// # }
    /// ```
    fn is_ancestor_of(&self, other: &Self) -> bool;

    /// Returns the Path that, were it joined to `base`, would yield `self`.
    /// If no such path exists, None is returned.
    /// If `self` is absolute and `base` is relative, or on Windows if both
    /// paths refer to separate drives, an absolute path is returned.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("foo/bar/baz/quux.txt");
    /// let fb = Path::new("foo/bar");
    /// let bq = Path::new("baz/quux.txt");
    /// assert!(p.path_relative_from(&fb) == Some(bq));
    /// # }
    /// ```
    fn path_relative_from(&self, base: &Self) -> Option<Self>;

    /// Returns whether the relative path `child` is a suffix of `self`.
    ///
    /// # Example
    ///
    /// ```
    /// # foo();
    /// # #[cfg(windows)] fn foo() {}
    /// # #[cfg(unix)] fn foo() {
    /// let p = Path::new("foo/bar/baz/quux.txt");
    /// let bq = Path::new("baz/quux.txt");
    /// assert!(p.ends_with_path(&bq));
    /// # }
    /// ```
    fn ends_with_path(&self, child: &Self) -> bool;
}

/// A trait that represents something bytes-like (e.g. a &[u8] or a &str)
pub trait BytesContainer for Sized? {
    /// Returns a &[u8] representing the receiver
    fn container_as_bytes<'a>(&'a self) -> &'a [u8];
    /// Returns the receiver interpreted as a utf-8 string, if possible
    #[inline]
    fn container_as_str<'a>(&'a self) -> Option<&'a str> { unimplemented!() }
    /// Returns whether .container_as_str() is guaranteed to not fail
    // FIXME (#8888): Remove unused arg once ::<for T> works
    #[inline]
    fn is_str(_: Option<&Self>) -> bool { unimplemented!() }
}

/// A trait that represents the unsafe operations on GenericPaths
pub trait GenericPathUnsafe {
    /// Creates a new Path without checking for null bytes.
    /// The resulting Path will always be normalized.
    unsafe fn new_unchecked<T: BytesContainer>(path: T) -> Self;

    /// Replaces the filename portion of the path without checking for null
    /// bytes.
    /// See `set_filename` for details.
    unsafe fn set_filename_unchecked<T: BytesContainer>(&mut self, filename: T);

    /// Pushes a path onto `self` without checking for null bytes.
    /// See `push` for details.
    unsafe fn push_unchecked<T: BytesContainer>(&mut self, path: T);
}

/// Helper struct for printing paths with format!()
pub struct Display<'a, P:'a> {
    path: &'a P,
    filename: bool
}

impl<'a, P: GenericPath> fmt::Show for Display<'a, P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { unimplemented!() }
}

impl<'a, P: GenericPath> Display<'a, P> {
    /// Returns the path as a possibly-owned string.
    ///
    /// If the path is not UTF-8, invalid sequences will be replaced with the
    /// Unicode replacement char. This involves allocation.
    #[inline]
    pub fn as_maybe_owned(&self) -> MaybeOwned<'a> { unimplemented!() }
}

impl BytesContainer for str {
    #[inline]
    fn container_as_bytes(&self) -> &[u8] { unimplemented!() }
    #[inline]
    fn container_as_str(&self) -> Option<&str> { unimplemented!() }
    #[inline]
    fn is_str(_: Option<&str>) -> bool { unimplemented!() }
}

impl BytesContainer for String {
    #[inline]
    fn container_as_bytes(&self) -> &[u8] { unimplemented!() }
    #[inline]
    fn container_as_str(&self) -> Option<&str> { unimplemented!() }
    #[inline]
    fn is_str(_: Option<&String>) -> bool { unimplemented!() }
}

impl BytesContainer for [u8] {
    #[inline]
    fn container_as_bytes(&self) -> &[u8] { unimplemented!() }
}

impl BytesContainer for Vec<u8> {
    #[inline]
    fn container_as_bytes(&self) -> &[u8] { unimplemented!() }
}

impl BytesContainer for CString {
    #[inline]
    fn container_as_bytes<'a>(&'a self) -> &'a [u8] { unimplemented!() }
}

impl<'a> BytesContainer for str::MaybeOwned<'a> {
    #[inline]
    fn container_as_bytes<'b>(&'b self) -> &'b [u8] { unimplemented!() }
    #[inline]
    fn container_as_str<'b>(&'b self) -> Option<&'b str> { unimplemented!() }
    #[inline]
    fn is_str(_: Option<&str::MaybeOwned>) -> bool { unimplemented!() }
}

impl<'a, Sized? T: BytesContainer> BytesContainer for &'a T {
    #[inline]
    fn container_as_bytes(&self) -> &[u8] { unimplemented!() }
    #[inline]
    fn container_as_str(&self) -> Option<&str> { unimplemented!() }
    #[inline]
    fn is_str(_: Option<& &'a T>) -> bool { unimplemented!() }
}

#[inline(always)]
fn contains_nul<T: BytesContainer>(v: &T) -> bool { unimplemented!() }
