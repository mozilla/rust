// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! POSIX file path handling

use container::Container;
use c_str::{CString, ToCStr};
use clone::Clone;
use cmp::{Eq, TotalEq};
use from_str::FromStr;
use io::Writer;
use iter::{AdditiveIterator, Extendable, Iterator, Map};
use option::{Option, None, Some};
use str;
use str::Str;
use slice::{CloneableVector, RevSplits, Splits, Vector, VectorVector,
            ImmutableEqVector, OwnedVector, ImmutableVector, OwnedCloneableVector};
use vec::Vec;

use super::{BytesContainer, GenericPath, GenericPathUnsafe};

/// Iterator that yields successive components of a Path as &[u8]
pub type Components<'a> = Splits<'a, u8>;
/// Iterator that yields components of a Path in reverse as &[u8]
pub type RevComponents<'a> = RevSplits<'a, u8>;

/// Iterator that yields successive components of a Path as Option<&str>
pub type StrComponents<'a> = Map<'a, &'a [u8], Option<&'a str>,
                                       Components<'a>>;
/// Iterator that yields components of a Path in reverse as Option<&str>
pub type RevStrComponents<'a> = Map<'a, &'a [u8], Option<&'a str>,
                                          RevComponents<'a>>;

/// Represents a POSIX file path
#[deriving(Clone)]
pub struct Path {
    repr: Vec<u8>, // assumed to never be empty or contain NULs
    sepidx: Option<uint> // index of the final separator in repr
}

/// The standard path separator character
pub static SEP: char = '/';

/// The standard path separator byte
pub static SEP_BYTE: u8 = SEP as u8;

/// Returns whether the given byte is a path separator
#[inline]
pub fn is_sep_byte(u: &u8) -> bool {
    *u as char == SEP
}

/// Returns whether the given char is a path separator
#[inline]
pub fn is_sep(c: char) -> bool {
    c == SEP
}

impl Eq for Path {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        self.repr == other.repr
    }
}

impl TotalEq for Path {}

impl FromStr for Path {
    fn from_str(s: &str) -> Option<Path> {
        Path::new_opt(s)
    }
}

impl ToCStr for Path {
    #[inline]
    fn to_c_str(&self) -> CString {
        // The Path impl guarantees no internal NUL
        unsafe { self.as_vec().to_c_str_unchecked() }
    }

    #[inline]
    unsafe fn to_c_str_unchecked(&self) -> CString {
        self.as_vec().to_c_str_unchecked()
    }
}

impl<S: Writer> ::hash::Hash<S> for Path {
    #[inline]
    fn hash(&self, state: &mut S) {
        self.repr.hash(state)
    }
}

impl BytesContainer for Path {
    #[inline]
    fn container_as_bytes<'a>(&'a self) -> &'a [u8] {
        self.as_vec()
    }
    #[inline]
    fn container_into_owned_bytes(self) -> Vec<u8> {
        self.into_vec()
    }
}

impl<'a> BytesContainer for &'a Path {
    #[inline]
    fn container_as_bytes<'a>(&'a self) -> &'a [u8] {
        self.as_vec()
    }
}

impl GenericPathUnsafe for Path {
    unsafe fn new_unchecked<T: BytesContainer>(path: T) -> Path {
        let path = Path::normalize(path.container_as_bytes());
        assert!(!path.is_empty());
        let idx = path.as_slice().rposition_elem(&SEP_BYTE);
        Path{ repr: path, sepidx: idx }
    }

    unsafe fn set_filename_unchecked<T: BytesContainer>(&mut self, filename: T) {
        let filename = filename.container_as_bytes();
        match self.sepidx {
            None if bytes!("..") == self.repr.as_slice() => {
                let mut v = Vec::with_capacity(3 + filename.len());
                v.push_all(dot_dot_static);
                v.push(SEP_BYTE);
                v.push_all(filename);
                // FIXME: this is slow
                self.repr = Path::normalize(v.as_slice());
            }
            None => {
                self.repr = Path::normalize(filename);
            }
            Some(idx) if self.repr.slice_from(idx+1) == bytes!("..") => {
                let mut v = Vec::with_capacity(self.repr.len() + 1 + filename.len());
                v.push_all(self.repr.as_slice());
                v.push(SEP_BYTE);
                v.push_all(filename);
                // FIXME: this is slow
                self.repr = Path::normalize(v.as_slice());
            }
            Some(idx) => {
                let mut v = Vec::with_capacity(idx + 1 + filename.len());
                v.push_all(self.repr.slice_to(idx+1));
                v.push_all(filename);
                // FIXME: this is slow
                self.repr = Path::normalize(v.as_slice());
            }
        }
        self.sepidx = self.repr.as_slice().rposition_elem(&SEP_BYTE);
    }

    unsafe fn push_unchecked<T: BytesContainer>(&mut self, path: T) {
        let path = path.container_as_bytes();
        if !path.is_empty() {
            if path[0] == SEP_BYTE {
                self.repr = Path::normalize(path);
            }  else {
                let mut v = Vec::with_capacity(self.repr.len() + path.len() + 1);
                v.push_all(self.repr.as_slice());
                v.push(SEP_BYTE);
                v.push_all(path);
                // FIXME: this is slow
                self.repr = Path::normalize(v.as_slice());
            }
            self.sepidx = self.repr.as_slice().rposition_elem(&SEP_BYTE);
        }
    }
}

impl GenericPath for Path {
    #[inline]
    fn as_vec<'a>(&'a self) -> &'a [u8] {
        self.repr.as_slice()
    }

    fn into_vec(self) -> Vec<u8> {
        self.repr
    }

    fn dirname<'a>(&'a self) -> &'a [u8] {
        match self.sepidx {
            None if bytes!("..") == self.repr.as_slice() => self.repr.as_slice(),
            None => dot_static,
            Some(0) => self.repr.slice_to(1),
            Some(idx) if self.repr.slice_from(idx+1) == bytes!("..") => self.repr.as_slice(),
            Some(idx) => self.repr.slice_to(idx)
        }
    }

    fn filename<'a>(&'a self) -> Option<&'a [u8]> {
        match self.sepidx {
            None if bytes!(".") == self.repr.as_slice() ||
                bytes!("..") == self.repr.as_slice() => None,
            None => Some(self.repr.as_slice()),
            Some(idx) if self.repr.slice_from(idx+1) == bytes!("..") => None,
            Some(0) if self.repr.slice_from(1).is_empty() => None,
            Some(idx) => Some(self.repr.slice_from(idx+1))
        }
    }

    fn pop(&mut self) -> bool {
        match self.sepidx {
            None if bytes!(".") == self.repr.as_slice() => false,
            None => {
                self.repr = vec!['.' as u8];
                self.sepidx = None;
                true
            }
            Some(0) if bytes!("/") == self.repr.as_slice() => false,
            Some(idx) => {
                if idx == 0 {
                    self.repr.truncate(idx+1);
                } else {
                    self.repr.truncate(idx);
                }
                self.sepidx = self.repr.as_slice().rposition_elem(&SEP_BYTE);
                true
            }
        }
    }

    fn root_path(&self) -> Option<Path> {
        if self.is_absolute() {
            Some(Path::new("/"))
        } else {
            None
        }
    }

    #[inline]
    fn is_absolute(&self) -> bool {
        *self.repr.get(0) == SEP_BYTE
    }

    fn is_ancestor_of(&self, other: &Path) -> bool {
        if self.is_absolute() != other.is_absolute() {
            false
        } else {
            let mut ita = self.components();
            let mut itb = other.components();
            if bytes!(".") == self.repr.as_slice() {
                return match itb.next() {
                    None => true,
                    Some(b) => b != bytes!("..")
                };
            }
            loop {
                match (ita.next(), itb.next()) {
                    (None, _) => break,
                    (Some(a), Some(b)) if a == b => { continue },
                    (Some(a), _) if a == bytes!("..") => {
                        // if ita contains only .. components, it's an ancestor
                        return ita.all(|x| x == bytes!(".."));
                    }
                    _ => return false
                }
            }
            true
        }
    }

    #[allow(deprecated_owned_vector)]
    fn path_relative_from(&self, base: &Path) -> Option<Path> {
        if self.is_absolute() != base.is_absolute() {
            if self.is_absolute() {
                Some(self.clone())
            } else {
                None
            }
        } else {
            let mut ita = self.components();
            let mut itb = base.components();
            let mut comps = vec![];
            loop {
                match (ita.next(), itb.next()) {
                    (None, None) => break,
                    (Some(a), None) => {
                        comps.push(a);
                        comps.extend(ita.by_ref());
                        break;
                    }
                    (None, _) => comps.push(dot_dot_static),
                    (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                    (Some(a), Some(b)) if b == bytes!(".") => comps.push(a),
                    (Some(_), Some(b)) if b == bytes!("..") => return None,
                    (Some(a), Some(_)) => {
                        comps.push(dot_dot_static);
                        for _ in itb {
                            comps.push(dot_dot_static);
                        }
                        comps.push(a);
                        comps.extend(ita.by_ref());
                        break;
                    }
                }
            }
            Some(Path::new(comps.as_slice().connect_vec(&SEP_BYTE)))
        }
    }

    fn ends_with_path(&self, child: &Path) -> bool {
        if !child.is_relative() { return false; }
        let mut selfit = self.rev_components();
        let mut childit = child.rev_components();
        loop {
            match (selfit.next(), childit.next()) {
                (Some(a), Some(b)) => if a != b { return false; },
                (Some(_), None) => break,
                (None, Some(_)) => return false,
                (None, None) => break
            }
        }
        true
    }
}

impl Path {
    /// Returns a new Path from a byte vector or string
    ///
    /// # Failure
    ///
    /// Fails the task if the vector contains a NUL.
    #[inline]
    pub fn new<T: BytesContainer>(path: T) -> Path {
        GenericPath::new(path)
    }

    /// Returns a new Path from a byte vector or string, if possible
    #[inline]
    pub fn new_opt<T: BytesContainer>(path: T) -> Option<Path> {
        GenericPath::new_opt(path)
    }

    /// Returns a normalized byte vector representation of a path, by removing all empty
    /// components, and unnecessary . and .. components.
    fn normalize<V: Vector<u8>+CloneableVector<u8>>(v: V) -> Vec<u8> {
        // borrowck is being very picky
        let val = {
            let is_abs = !v.as_slice().is_empty() && v.as_slice()[0] == SEP_BYTE;
            let v_ = if is_abs { v.as_slice().slice_from(1) } else { v.as_slice() };
            let comps = normalize_helper(v_, is_abs);
            match comps {
                None => None,
                Some(comps) => {
                    if is_abs && comps.is_empty() {
                        Some(vec![SEP_BYTE])
                    } else {
                        let n = if is_abs { comps.len() } else { comps.len() - 1} +
                                comps.iter().map(|v| v.len()).sum();
                        let mut v = Vec::with_capacity(n);
                        let mut it = comps.move_iter();
                        if !is_abs {
                            match it.next() {
                                None => (),
                                Some(comp) => v.push_all(comp)
                            }
                        }
                        for comp in it {
                            v.push(SEP_BYTE);
                            v.push_all(comp);
                        }
                        Some(v)
                    }
                }
            }
        };
        match val {
            None => Vec::from_slice(v.as_slice()),
            Some(val) => val
        }
    }

    /// Returns an iterator that yields each component of the path in turn.
    /// Does not distinguish between absolute and relative paths, e.g.
    /// /a/b/c and a/b/c yield the same set of components.
    /// A path of "/" yields no components. A path of "." yields one component.
    pub fn components<'a>(&'a self) -> Components<'a> {
        let v = if *self.repr.get(0) == SEP_BYTE {
            self.repr.slice_from(1)
        } else { self.repr.as_slice() };
        let mut ret = v.split(is_sep_byte);
        if v.is_empty() {
            // consume the empty "" component
            ret.next();
        }
        ret
    }

    /// Returns an iterator that yields each component of the path in reverse.
    /// See components() for details.
    pub fn rev_components<'a>(&'a self) -> RevComponents<'a> {
        let v = if *self.repr.get(0) == SEP_BYTE {
            self.repr.slice_from(1)
        } else { self.repr.as_slice() };
        let mut ret = v.rsplit(is_sep_byte);
        if v.is_empty() {
            // consume the empty "" component
            ret.next();
        }
        ret
    }

    /// Returns an iterator that yields each component of the path as Option<&str>.
    /// See components() for details.
    pub fn str_components<'a>(&'a self) -> StrComponents<'a> {
        self.components().map(str::from_utf8)
    }

    /// Returns an iterator that yields each component of the path in reverse as Option<&str>.
    /// See components() for details.
    pub fn rev_str_components<'a>(&'a self) -> RevStrComponents<'a> {
        self.rev_components().map(str::from_utf8)
    }
}

// None result means the byte vector didn't need normalizing
fn normalize_helper<'a>(v: &'a [u8], is_abs: bool) -> Option<Vec<&'a [u8]>> {
    if is_abs && v.as_slice().is_empty() {
        return None;
    }
    let mut comps: Vec<&'a [u8]> = vec![];
    let mut n_up = 0u;
    let mut changed = false;
    for comp in v.split(is_sep_byte) {
        if comp.is_empty() { changed = true }
        else if comp == bytes!(".") { changed = true }
        else if comp == bytes!("..") {
            if is_abs && comps.is_empty() { changed = true }
            else if comps.len() == n_up { comps.push(dot_dot_static); n_up += 1 }
            else { comps.pop().unwrap(); changed = true }
        } else { comps.push(comp) }
    }
    if changed {
        if comps.is_empty() && !is_abs {
            if v == bytes!(".") {
                return None;
            }
            comps.push(dot_static);
        }
        Some(comps)
    } else {
        None
    }
}

static dot_static: &'static [u8] = bytes!(".");
static dot_dot_static: &'static [u8] = bytes!("..");

#[cfg(test)]
mod tests {
    use prelude::*;
    use super::*;
    use str;
    use str::StrSlice;

    macro_rules! t(
        (s: $path:expr, $exp:expr) => (
            {
                let path = $path;
                assert!(path.as_str() == Some($exp));
            }
        );
        (v: $path:expr, $exp:expr) => (
            {
                let path = $path;
                assert!(path.as_vec() == $exp);
            }
        )
    )

    macro_rules! b(
        ($($arg:expr),+) => (
            {
                static the_bytes: &'static [u8] = bytes!($($arg),+);
                the_bytes
            }
        )
    )

    #[test]
    fn test_paths() {
        let empty: &[u8] = [];
        t!(v: Path::new(empty), b!("."));
        t!(v: Path::new(b!("/")), b!("/"));
        t!(v: Path::new(b!("a/b/c")), b!("a/b/c"));
        t!(v: Path::new(b!("a/b/c", 0xff)), b!("a/b/c", 0xff));
        t!(v: Path::new(b!(0xff, "/../foo", 0x80)), b!("foo", 0x80));
        let p = Path::new(b!("a/b/c", 0xff));
        assert!(p.as_str() == None);

        t!(s: Path::new(""), ".");
        t!(s: Path::new("/"), "/");
        t!(s: Path::new("hi"), "hi");
        t!(s: Path::new("hi/"), "hi");
        t!(s: Path::new("/lib"), "/lib");
        t!(s: Path::new("/lib/"), "/lib");
        t!(s: Path::new("hi/there"), "hi/there");
        t!(s: Path::new("hi/there.txt"), "hi/there.txt");

        t!(s: Path::new("hi/there/"), "hi/there");
        t!(s: Path::new("hi/../there"), "there");
        t!(s: Path::new("../hi/there"), "../hi/there");
        t!(s: Path::new("/../hi/there"), "/hi/there");
        t!(s: Path::new("foo/.."), ".");
        t!(s: Path::new("/foo/.."), "/");
        t!(s: Path::new("/foo/../.."), "/");
        t!(s: Path::new("/foo/../../bar"), "/bar");
        t!(s: Path::new("/./hi/./there/."), "/hi/there");
        t!(s: Path::new("/./hi/./there/./.."), "/hi");
        t!(s: Path::new("foo/../.."), "..");
        t!(s: Path::new("foo/../../.."), "../..");
        t!(s: Path::new("foo/../../bar"), "../bar");

        assert_eq!(Path::new(b!("foo/bar")).into_vec().as_slice(), b!("foo/bar"));
        assert_eq!(Path::new(b!("/foo/../../bar")).into_vec().as_slice(),
                   b!("/bar"));

        let p = Path::new(b!("foo/bar", 0x80));
        assert!(p.as_str() == None);
    }

    #[test]
    fn test_opt_paths() {
        assert!(Path::new_opt(b!("foo/bar", 0)) == None);
        t!(v: Path::new_opt(b!("foo/bar")).unwrap(), b!("foo/bar"));
        assert!(Path::new_opt("foo/bar\0") == None);
        t!(s: Path::new_opt("foo/bar").unwrap(), "foo/bar");
    }

    #[test]
    fn test_null_byte() {
        use task;
        let result = task::try(proc() {
            Path::new(b!("foo/bar", 0))
        });
        assert!(result.is_err());

        let result = task::try(proc() {
            Path::new("test").set_filename(b!("f", 0, "o"))
        });
        assert!(result.is_err());

        let result = task::try(proc() {
            Path::new("test").push(b!("f", 0, "o"));
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_display_str() {
        macro_rules! t(
            ($path:expr, $disp:ident, $exp:expr) => (
                {
                    let path = Path::new($path);
                    assert!(path.$disp().to_str() == ~$exp);
                }
            )
        )
        t!("foo", display, "foo");
        t!(b!("foo", 0x80), display, "foo\uFFFD");
        t!(b!("foo", 0xff, "bar"), display, "foo\uFFFDbar");
        t!(b!("foo", 0xff, "/bar"), filename_display, "bar");
        t!(b!("foo/", 0xff, "bar"), filename_display, "\uFFFDbar");
        t!(b!("/"), filename_display, "");

        macro_rules! t(
            ($path:expr, $exp:expr) => (
                {
                    let path = Path::new($path);
                    let mo = path.display().as_maybe_owned();
                    assert!(mo.as_slice() == $exp);
                }
            );
            ($path:expr, $exp:expr, filename) => (
                {
                    let path = Path::new($path);
                    let mo = path.filename_display().as_maybe_owned();
                    assert!(mo.as_slice() == $exp);
                }
            )
        )

        t!("foo", "foo");
        t!(b!("foo", 0x80), "foo\uFFFD");
        t!(b!("foo", 0xff, "bar"), "foo\uFFFDbar");
        t!(b!("foo", 0xff, "/bar"), "bar", filename);
        t!(b!("foo/", 0xff, "bar"), "\uFFFDbar", filename);
        t!(b!("/"), "", filename);
    }

    #[test]
    fn test_display() {
        macro_rules! t(
            ($path:expr, $exp:expr, $expf:expr) => (
                {
                    let path = Path::new($path);
                    let f = format!("{}", path.display());
                    assert!(f.as_slice() == $exp);
                    let f = format!("{}", path.filename_display());
                    assert!(f.as_slice() == $expf);
                }
            )
        )

        t!(b!("foo"), "foo", "foo");
        t!(b!("foo/bar"), "foo/bar", "bar");
        t!(b!("/"), "/", "");
        t!(b!("foo", 0xff), "foo\uFFFD", "foo\uFFFD");
        t!(b!("foo", 0xff, "/bar"), "foo\uFFFD/bar", "bar");
        t!(b!("foo/", 0xff, "bar"), "foo/\uFFFDbar", "\uFFFDbar");
        t!(b!(0xff, "foo/bar", 0xff), "\uFFFDfoo/bar\uFFFD", "bar\uFFFD");
    }

    #[test]
    fn test_components() {
        macro_rules! t(
            (s: $path:expr, $op:ident, $exp:expr) => (
                {
                    let path = Path::new($path);
                    assert!(path.$op() == ($exp).as_bytes());
                }
            );
            (s: $path:expr, $op:ident, $exp:expr, opt) => (
                {
                    let path = Path::new($path);
                    let left = path.$op().map(|x| str::from_utf8(x).unwrap());
                    assert!(left == $exp);
                }
            );
            (v: $path:expr, $op:ident, $exp:expr) => (
                {
                    let arg = $path;
                    let path = Path::new(arg);
                    assert!(path.$op() == $exp);
                }
            );
        )

        t!(v: b!("a/b/c"), filename, Some(b!("c")));
        t!(v: b!("a/b/c", 0xff), filename, Some(b!("c", 0xff)));
        t!(v: b!("a/b", 0xff, "/c"), filename, Some(b!("c")));
        t!(s: "a/b/c", filename, Some("c"), opt);
        t!(s: "/a/b/c", filename, Some("c"), opt);
        t!(s: "a", filename, Some("a"), opt);
        t!(s: "/a", filename, Some("a"), opt);
        t!(s: ".", filename, None, opt);
        t!(s: "/", filename, None, opt);
        t!(s: "..", filename, None, opt);
        t!(s: "../..", filename, None, opt);

        t!(v: b!("a/b/c"), dirname, b!("a/b"));
        t!(v: b!("a/b/c", 0xff), dirname, b!("a/b"));
        t!(v: b!("a/b", 0xff, "/c"), dirname, b!("a/b", 0xff));
        t!(s: "a/b/c", dirname, "a/b");
        t!(s: "/a/b/c", dirname, "/a/b");
        t!(s: "a", dirname, ".");
        t!(s: "/a", dirname, "/");
        t!(s: ".", dirname, ".");
        t!(s: "/", dirname, "/");
        t!(s: "..", dirname, "..");
        t!(s: "../..", dirname, "../..");

        t!(v: b!("hi/there.txt"), filestem, Some(b!("there")));
        t!(v: b!("hi/there", 0x80, ".txt"), filestem, Some(b!("there", 0x80)));
        t!(v: b!("hi/there.t", 0x80, "xt"), filestem, Some(b!("there")));
        t!(s: "hi/there.txt", filestem, Some("there"), opt);
        t!(s: "hi/there", filestem, Some("there"), opt);
        t!(s: "there.txt", filestem, Some("there"), opt);
        t!(s: "there", filestem, Some("there"), opt);
        t!(s: ".", filestem, None, opt);
        t!(s: "/", filestem, None, opt);
        t!(s: "foo/.bar", filestem, Some(".bar"), opt);
        t!(s: ".bar", filestem, Some(".bar"), opt);
        t!(s: "..bar", filestem, Some("."), opt);
        t!(s: "hi/there..txt", filestem, Some("there."), opt);
        t!(s: "..", filestem, None, opt);
        t!(s: "../..", filestem, None, opt);

        t!(v: b!("hi/there.txt"), extension, Some(b!("txt")));
        t!(v: b!("hi/there", 0x80, ".txt"), extension, Some(b!("txt")));
        t!(v: b!("hi/there.t", 0x80, "xt"), extension, Some(b!("t", 0x80, "xt")));
        t!(v: b!("hi/there"), extension, None);
        t!(v: b!("hi/there", 0x80), extension, None);
        t!(s: "hi/there.txt", extension, Some("txt"), opt);
        t!(s: "hi/there", extension, None, opt);
        t!(s: "there.txt", extension, Some("txt"), opt);
        t!(s: "there", extension, None, opt);
        t!(s: ".", extension, None, opt);
        t!(s: "/", extension, None, opt);
        t!(s: "foo/.bar", extension, None, opt);
        t!(s: ".bar", extension, None, opt);
        t!(s: "..bar", extension, Some("bar"), opt);
        t!(s: "hi/there..txt", extension, Some("txt"), opt);
        t!(s: "..", extension, None, opt);
        t!(s: "../..", extension, None, opt);
    }

    #[test]
    fn test_push() {
        macro_rules! t(
            (s: $path:expr, $join:expr) => (
                {
                    let path = $path;
                    let join = $join;
                    let mut p1 = Path::new(path);
                    let p2 = p1.clone();
                    p1.push(join);
                    assert!(p1 == p2.join(join));
                }
            )
        )

        t!(s: "a/b/c", "..");
        t!(s: "/a/b/c", "d");
        t!(s: "a/b", "c/d");
        t!(s: "a/b", "/c/d");
    }

    #[test]
    fn test_push_path() {
        macro_rules! t(
            (s: $path:expr, $push:expr, $exp:expr) => (
                {
                    let mut p = Path::new($path);
                    let push = Path::new($push);
                    p.push(&push);
                    assert!(p.as_str() == Some($exp));
                }
            )
        )

        t!(s: "a/b/c", "d", "a/b/c/d");
        t!(s: "/a/b/c", "d", "/a/b/c/d");
        t!(s: "a/b", "c/d", "a/b/c/d");
        t!(s: "a/b", "/c/d", "/c/d");
        t!(s: "a/b", ".", "a/b");
        t!(s: "a/b", "../c", "a/c");
    }

    #[test]
    fn test_push_many() {
        macro_rules! t(
            (s: $path:expr, $push:expr, $exp:expr) => (
                {
                    let mut p = Path::new($path);
                    p.push_many($push);
                    assert!(p.as_str() == Some($exp));
                }
            );
            (v: $path:expr, $push:expr, $exp:expr) => (
                {
                    let mut p = Path::new($path);
                    p.push_many($push);
                    assert!(p.as_vec() == $exp);
                }
            )
        )

        t!(s: "a/b/c", ["d", "e"], "a/b/c/d/e");
        t!(s: "a/b/c", ["d", "/e"], "/e");
        t!(s: "a/b/c", ["d", "/e", "f"], "/e/f");
        t!(s: "a/b/c", ["d".to_owned(), "e".to_owned()], "a/b/c/d/e");
        t!(v: b!("a/b/c"), [b!("d"), b!("e")], b!("a/b/c/d/e"));
        t!(v: b!("a/b/c"), [b!("d"), b!("/e"), b!("f")], b!("/e/f"));
        t!(v: b!("a/b/c"), [Vec::from_slice(b!("d")), Vec::from_slice(b!("e"))], b!("a/b/c/d/e"));
    }

    #[test]
    fn test_pop() {
        macro_rules! t(
            (s: $path:expr, $left:expr, $right:expr) => (
                {
                    let mut p = Path::new($path);
                    let result = p.pop();
                    assert!(p.as_str() == Some($left));
                    assert!(result == $right);
                }
            );
            (v: [$($path:expr),+], [$($left:expr),+], $right:expr) => (
                {
                    let mut p = Path::new(b!($($path),+));
                    let result = p.pop();
                    assert!(p.as_vec() == b!($($left),+));
                    assert!(result == $right);
                }
            )
        )

        t!(v: ["a/b/c"], ["a/b"], true);
        t!(v: ["a"], ["."], true);
        t!(v: ["."], ["."], false);
        t!(v: ["/a"], ["/"], true);
        t!(v: ["/"], ["/"], false);
        t!(v: ["a/b/c", 0x80], ["a/b"], true);
        t!(v: ["a/b", 0x80, "/c"], ["a/b", 0x80], true);
        t!(v: [0xff], ["."], true);
        t!(v: ["/", 0xff], ["/"], true);
        t!(s: "a/b/c", "a/b", true);
        t!(s: "a", ".", true);
        t!(s: ".", ".", false);
        t!(s: "/a", "/", true);
        t!(s: "/", "/", false);
    }

    #[test]
    fn test_root_path() {
        assert!(Path::new(b!("a/b/c")).root_path() == None);
        assert!(Path::new(b!("/a/b/c")).root_path() == Some(Path::new("/")));
    }

    #[test]
    fn test_join() {
        t!(v: Path::new(b!("a/b/c")).join(b!("..")), b!("a/b"));
        t!(v: Path::new(b!("/a/b/c")).join(b!("d")), b!("/a/b/c/d"));
        t!(v: Path::new(b!("a/", 0x80, "/c")).join(b!(0xff)), b!("a/", 0x80, "/c/", 0xff));
        t!(s: Path::new("a/b/c").join(".."), "a/b");
        t!(s: Path::new("/a/b/c").join("d"), "/a/b/c/d");
        t!(s: Path::new("a/b").join("c/d"), "a/b/c/d");
        t!(s: Path::new("a/b").join("/c/d"), "/c/d");
        t!(s: Path::new(".").join("a/b"), "a/b");
        t!(s: Path::new("/").join("a/b"), "/a/b");
    }

    #[test]
    fn test_join_path() {
        macro_rules! t(
            (s: $path:expr, $join:expr, $exp:expr) => (
                {
                    let path = Path::new($path);
                    let join = Path::new($join);
                    let res = path.join(&join);
                    assert!(res.as_str() == Some($exp));
                }
            )
        )

        t!(s: "a/b/c", "..", "a/b");
        t!(s: "/a/b/c", "d", "/a/b/c/d");
        t!(s: "a/b", "c/d", "a/b/c/d");
        t!(s: "a/b", "/c/d", "/c/d");
        t!(s: ".", "a/b", "a/b");
        t!(s: "/", "a/b", "/a/b");
    }

    #[test]
    fn test_join_many() {
        macro_rules! t(
            (s: $path:expr, $join:expr, $exp:expr) => (
                {
                    let path = Path::new($path);
                    let res = path.join_many($join);
                    assert!(res.as_str() == Some($exp));
                }
            );
            (v: $path:expr, $join:expr, $exp:expr) => (
                {
                    let path = Path::new($path);
                    let res = path.join_many($join);
                    assert!(res.as_vec() == $exp);
                }
            )
        )

        t!(s: "a/b/c", ["d", "e"], "a/b/c/d/e");
        t!(s: "a/b/c", ["..", "d"], "a/b/d");
        t!(s: "a/b/c", ["d", "/e", "f"], "/e/f");
        t!(s: "a/b/c", ["d".to_owned(), "e".to_owned()], "a/b/c/d/e");
        t!(v: b!("a/b/c"), [b!("d"), b!("e")], b!("a/b/c/d/e"));
        t!(v: b!("a/b/c"), [Vec::from_slice(b!("d")), Vec::from_slice(b!("e"))], b!("a/b/c/d/e"));
    }

    #[test]
    fn test_with_helpers() {
        let empty: &[u8] = [];

        t!(v: Path::new(b!("a/b/c")).with_filename(b!("d")), b!("a/b/d"));
        t!(v: Path::new(b!("a/b/c", 0xff)).with_filename(b!(0x80)), b!("a/b/", 0x80));
        t!(v: Path::new(b!("/", 0xff, "/foo")).with_filename(b!(0xcd)),
              b!("/", 0xff, "/", 0xcd));
        t!(s: Path::new("a/b/c").with_filename("d"), "a/b/d");
        t!(s: Path::new(".").with_filename("foo"), "foo");
        t!(s: Path::new("/a/b/c").with_filename("d"), "/a/b/d");
        t!(s: Path::new("/").with_filename("foo"), "/foo");
        t!(s: Path::new("/a").with_filename("foo"), "/foo");
        t!(s: Path::new("foo").with_filename("bar"), "bar");
        t!(s: Path::new("/").with_filename("foo/"), "/foo");
        t!(s: Path::new("/a").with_filename("foo/"), "/foo");
        t!(s: Path::new("a/b/c").with_filename(""), "a/b");
        t!(s: Path::new("a/b/c").with_filename("."), "a/b");
        t!(s: Path::new("a/b/c").with_filename(".."), "a");
        t!(s: Path::new("/a").with_filename(""), "/");
        t!(s: Path::new("foo").with_filename(""), ".");
        t!(s: Path::new("a/b/c").with_filename("d/e"), "a/b/d/e");
        t!(s: Path::new("a/b/c").with_filename("/d"), "a/b/d");
        t!(s: Path::new("..").with_filename("foo"), "../foo");
        t!(s: Path::new("../..").with_filename("foo"), "../../foo");
        t!(s: Path::new("..").with_filename(""), "..");
        t!(s: Path::new("../..").with_filename(""), "../..");

        t!(v: Path::new(b!("hi/there", 0x80, ".txt")).with_extension(b!("exe")),
              b!("hi/there", 0x80, ".exe"));
        t!(v: Path::new(b!("hi/there.txt", 0x80)).with_extension(b!(0xff)),
              b!("hi/there.", 0xff));
        t!(v: Path::new(b!("hi/there", 0x80)).with_extension(b!(0xff)),
              b!("hi/there", 0x80, ".", 0xff));
        t!(v: Path::new(b!("hi/there.", 0xff)).with_extension(empty), b!("hi/there"));
        t!(s: Path::new("hi/there.txt").with_extension("exe"), "hi/there.exe");
        t!(s: Path::new("hi/there.txt").with_extension(""), "hi/there");
        t!(s: Path::new("hi/there.txt").with_extension("."), "hi/there..");
        t!(s: Path::new("hi/there.txt").with_extension(".."), "hi/there...");
        t!(s: Path::new("hi/there").with_extension("txt"), "hi/there.txt");
        t!(s: Path::new("hi/there").with_extension("."), "hi/there..");
        t!(s: Path::new("hi/there").with_extension(".."), "hi/there...");
        t!(s: Path::new("hi/there.").with_extension("txt"), "hi/there.txt");
        t!(s: Path::new("hi/.foo").with_extension("txt"), "hi/.foo.txt");
        t!(s: Path::new("hi/there.txt").with_extension(".foo"), "hi/there..foo");
        t!(s: Path::new("/").with_extension("txt"), "/");
        t!(s: Path::new("/").with_extension("."), "/");
        t!(s: Path::new("/").with_extension(".."), "/");
        t!(s: Path::new(".").with_extension("txt"), ".");
    }

    #[test]
    fn test_setters() {
        macro_rules! t(
            (s: $path:expr, $set:ident, $with:ident, $arg:expr) => (
                {
                    let path = $path;
                    let arg = $arg;
                    let mut p1 = Path::new(path);
                    p1.$set(arg);
                    let p2 = Path::new(path);
                    assert!(p1 == p2.$with(arg));
                }
            );
            (v: $path:expr, $set:ident, $with:ident, $arg:expr) => (
                {
                    let path = $path;
                    let arg = $arg;
                    let mut p1 = Path::new(path);
                    p1.$set(arg);
                    let p2 = Path::new(path);
                    assert!(p1 == p2.$with(arg));
                }
            )
        )

        t!(v: b!("a/b/c"), set_filename, with_filename, b!("d"));
        t!(v: b!("/"), set_filename, with_filename, b!("foo"));
        t!(v: b!(0x80), set_filename, with_filename, b!(0xff));
        t!(s: "a/b/c", set_filename, with_filename, "d");
        t!(s: "/", set_filename, with_filename, "foo");
        t!(s: ".", set_filename, with_filename, "foo");
        t!(s: "a/b", set_filename, with_filename, "");
        t!(s: "a", set_filename, with_filename, "");

        t!(v: b!("hi/there.txt"), set_extension, with_extension, b!("exe"));
        t!(v: b!("hi/there.t", 0x80, "xt"), set_extension, with_extension, b!("exe", 0xff));
        t!(s: "hi/there.txt", set_extension, with_extension, "exe");
        t!(s: "hi/there.", set_extension, with_extension, "txt");
        t!(s: "hi/there", set_extension, with_extension, "txt");
        t!(s: "hi/there.txt", set_extension, with_extension, "");
        t!(s: "hi/there", set_extension, with_extension, "");
        t!(s: ".", set_extension, with_extension, "txt");
    }

    #[test]
    fn test_getters() {
        macro_rules! t(
            (s: $path:expr, $filename:expr, $dirname:expr, $filestem:expr, $ext:expr) => (
                {
                    let path = $path;
                    let filename = $filename;
                    assert!(path.filename_str() == filename,
                            "{}.filename_str(): Expected `{:?}`, found {:?}",
                            path.as_str().unwrap(), filename, path.filename_str());
                    let dirname = $dirname;
                    assert!(path.dirname_str() == dirname,
                            "`{}`.dirname_str(): Expected `{:?}`, found `{:?}`",
                            path.as_str().unwrap(), dirname, path.dirname_str());
                    let filestem = $filestem;
                    assert!(path.filestem_str() == filestem,
                            "`{}`.filestem_str(): Expected `{:?}`, found `{:?}`",
                            path.as_str().unwrap(), filestem, path.filestem_str());
                    let ext = $ext;
                    assert!(path.extension_str() == ext,
                            "`{}`.extension_str(): Expected `{:?}`, found `{:?}`",
                            path.as_str().unwrap(), ext, path.extension_str());
                }
            );
            (v: $path:expr, $filename:expr, $dirname:expr, $filestem:expr, $ext:expr) => (
                {
                    let path = $path;
                    assert!(path.filename() == $filename);
                    assert!(path.dirname() == $dirname);
                    assert!(path.filestem() == $filestem);
                    assert!(path.extension() == $ext);
                }
            )
        )

        t!(v: Path::new(b!("a/b/c")), Some(b!("c")), b!("a/b"), Some(b!("c")), None);
        t!(v: Path::new(b!("a/b/", 0xff)), Some(b!(0xff)), b!("a/b"), Some(b!(0xff)), None);
        t!(v: Path::new(b!("hi/there.", 0xff)), Some(b!("there.", 0xff)), b!("hi"),
              Some(b!("there")), Some(b!(0xff)));
        t!(s: Path::new("a/b/c"), Some("c"), Some("a/b"), Some("c"), None);
        t!(s: Path::new("."), None, Some("."), None, None);
        t!(s: Path::new("/"), None, Some("/"), None, None);
        t!(s: Path::new(".."), None, Some(".."), None, None);
        t!(s: Path::new("../.."), None, Some("../.."), None, None);
        t!(s: Path::new("hi/there.txt"), Some("there.txt"), Some("hi"),
              Some("there"), Some("txt"));
        t!(s: Path::new("hi/there"), Some("there"), Some("hi"), Some("there"), None);
        t!(s: Path::new("hi/there."), Some("there."), Some("hi"),
              Some("there"), Some(""));
        t!(s: Path::new("hi/.there"), Some(".there"), Some("hi"), Some(".there"), None);
        t!(s: Path::new("hi/..there"), Some("..there"), Some("hi"),
              Some("."), Some("there"));
        t!(s: Path::new(b!("a/b/", 0xff)), None, Some("a/b"), None, None);
        t!(s: Path::new(b!("a/b/", 0xff, ".txt")), None, Some("a/b"), None, Some("txt"));
        t!(s: Path::new(b!("a/b/c.", 0x80)), None, Some("a/b"), Some("c"), None);
        t!(s: Path::new(b!(0xff, "/b")), Some("b"), None, Some("b"), None);
    }

    #[test]
    fn test_dir_path() {
        t!(v: Path::new(b!("hi/there", 0x80)).dir_path(), b!("hi"));
        t!(v: Path::new(b!("hi", 0xff, "/there")).dir_path(), b!("hi", 0xff));
        t!(s: Path::new("hi/there").dir_path(), "hi");
        t!(s: Path::new("hi").dir_path(), ".");
        t!(s: Path::new("/hi").dir_path(), "/");
        t!(s: Path::new("/").dir_path(), "/");
        t!(s: Path::new("..").dir_path(), "..");
        t!(s: Path::new("../..").dir_path(), "../..");
    }

    #[test]
    fn test_is_absolute() {
        macro_rules! t(
            (s: $path:expr, $abs:expr, $rel:expr) => (
                {
                    let path = Path::new($path);
                    assert_eq!(path.is_absolute(), $abs);
                    assert_eq!(path.is_relative(), $rel);
                }
            )
        )
        t!(s: "a/b/c", false, true);
        t!(s: "/a/b/c", true, false);
        t!(s: "a", false, true);
        t!(s: "/a", true, false);
        t!(s: ".", false, true);
        t!(s: "/", true, false);
        t!(s: "..", false, true);
        t!(s: "../..", false, true);
    }

    #[test]
    fn test_is_ancestor_of() {
        macro_rules! t(
            (s: $path:expr, $dest:expr, $exp:expr) => (
                {
                    let path = Path::new($path);
                    let dest = Path::new($dest);
                    assert_eq!(path.is_ancestor_of(&dest), $exp);
                }
            )
        )

        t!(s: "a/b/c", "a/b/c/d", true);
        t!(s: "a/b/c", "a/b/c", true);
        t!(s: "a/b/c", "a/b", false);
        t!(s: "/a/b/c", "/a/b/c", true);
        t!(s: "/a/b", "/a/b/c", true);
        t!(s: "/a/b/c/d", "/a/b/c", false);
        t!(s: "/a/b", "a/b/c", false);
        t!(s: "a/b", "/a/b/c", false);
        t!(s: "a/b/c", "a/b/d", false);
        t!(s: "../a/b/c", "a/b/c", false);
        t!(s: "a/b/c", "../a/b/c", false);
        t!(s: "a/b/c", "a/b/cd", false);
        t!(s: "a/b/cd", "a/b/c", false);
        t!(s: "../a/b", "../a/b/c", true);
        t!(s: ".", "a/b", true);
        t!(s: ".", ".", true);
        t!(s: "/", "/", true);
        t!(s: "/", "/a/b", true);
        t!(s: "..", "a/b", true);
        t!(s: "../..", "a/b", true);
    }

    #[test]
    fn test_ends_with_path() {
        macro_rules! t(
            (s: $path:expr, $child:expr, $exp:expr) => (
                {
                    let path = Path::new($path);
                    let child = Path::new($child);
                    assert_eq!(path.ends_with_path(&child), $exp);
                }
            );
            (v: $path:expr, $child:expr, $exp:expr) => (
                {
                    let path = Path::new($path);
                    let child = Path::new($child);
                    assert_eq!(path.ends_with_path(&child), $exp);
                }
            )
        )

        t!(s: "a/b/c", "c", true);
        t!(s: "a/b/c", "d", false);
        t!(s: "foo/bar/quux", "bar", false);
        t!(s: "foo/bar/quux", "barquux", false);
        t!(s: "a/b/c", "b/c", true);
        t!(s: "a/b/c", "a/b/c", true);
        t!(s: "a/b/c", "foo/a/b/c", false);
        t!(s: "/a/b/c", "a/b/c", true);
        t!(s: "/a/b/c", "/a/b/c", false); // child must be relative
        t!(s: "/a/b/c", "foo/a/b/c", false);
        t!(s: "a/b/c", "", false);
        t!(s: "", "", true);
        t!(s: "/a/b/c", "d/e/f", false);
        t!(s: "a/b/c", "a/b", false);
        t!(s: "a/b/c", "b", false);
        t!(v: b!("a/b/c"), b!("b/c"), true);
        t!(v: b!("a/b/", 0xff), b!(0xff), true);
        t!(v: b!("a/b/", 0xff), b!("b/", 0xff), true);
    }

    #[test]
    fn test_path_relative_from() {
        macro_rules! t(
            (s: $path:expr, $other:expr, $exp:expr) => (
                {
                    let path = Path::new($path);
                    let other = Path::new($other);
                    let res = path.path_relative_from(&other);
                    assert_eq!(res.as_ref().and_then(|x| x.as_str()), $exp);
                }
            )
        )

        t!(s: "a/b/c", "a/b", Some("c"));
        t!(s: "a/b/c", "a/b/d", Some("../c"));
        t!(s: "a/b/c", "a/b/c/d", Some(".."));
        t!(s: "a/b/c", "a/b/c", Some("."));
        t!(s: "a/b/c", "a/b/c/d/e", Some("../.."));
        t!(s: "a/b/c", "a/d/e", Some("../../b/c"));
        t!(s: "a/b/c", "d/e/f", Some("../../../a/b/c"));
        t!(s: "a/b/c", "/a/b/c", None);
        t!(s: "/a/b/c", "a/b/c", Some("/a/b/c"));
        t!(s: "/a/b/c", "/a/b/c/d", Some(".."));
        t!(s: "/a/b/c", "/a/b", Some("c"));
        t!(s: "/a/b/c", "/a/b/c/d/e", Some("../.."));
        t!(s: "/a/b/c", "/a/d/e", Some("../../b/c"));
        t!(s: "/a/b/c", "/d/e/f", Some("../../../a/b/c"));
        t!(s: "hi/there.txt", "hi/there", Some("../there.txt"));
        t!(s: ".", "a", Some(".."));
        t!(s: ".", "a/b", Some("../.."));
        t!(s: ".", ".", Some("."));
        t!(s: "a", ".", Some("a"));
        t!(s: "a/b", ".", Some("a/b"));
        t!(s: "..", ".", Some(".."));
        t!(s: "a/b/c", "a/b/c", Some("."));
        t!(s: "/a/b/c", "/a/b/c", Some("."));
        t!(s: "/", "/", Some("."));
        t!(s: "/", ".", Some("/"));
        t!(s: "../../a", "b", Some("../../../a"));
        t!(s: "a", "../../b", None);
        t!(s: "../../a", "../../b", Some("../a"));
        t!(s: "../../a", "../../a/b", Some(".."));
        t!(s: "../../a/b", "../../a", Some("b"));
    }

    #[test]
    fn test_components_iter() {
        macro_rules! t(
            (s: $path:expr, $exp:expr) => (
                {
                    let path = Path::new($path);
                    let comps = path.components().collect::<Vec<&[u8]>>();
                    let exp: &[&str] = $exp;
                    let exps = exp.iter().map(|x| x.as_bytes()).collect::<Vec<&[u8]>>();
                    assert!(comps == exps, "components: Expected {:?}, found {:?}",
                            comps, exps);
                    let comps = path.rev_components().collect::<Vec<&[u8]>>();
                    let exps = exps.move_iter().rev().collect::<Vec<&[u8]>>();
                    assert!(comps == exps, "rev_components: Expected {:?}, found {:?}",
                            comps, exps);
                }
            );
            (v: [$($arg:expr),+], [$([$($exp:expr),*]),*]) => (
                {
                    let path = Path::new(b!($($arg),+));
                    let comps = path.components().collect::<Vec<&[u8]>>();
                    let exp: &[&[u8]] = [$(b!($($exp),*)),*];
                    assert_eq!(comps.as_slice(), exp);
                    let comps = path.rev_components().collect::<Vec<&[u8]>>();
                    let exp = exp.rev_iter().map(|&x|x).collect::<Vec<&[u8]>>();
                    assert_eq!(comps, exp)
                }
            )
        )

        t!(v: ["a/b/c"], [["a"], ["b"], ["c"]]);
        t!(v: ["/", 0xff, "/a/", 0x80], [[0xff], ["a"], [0x80]]);
        t!(v: ["../../foo", 0xcd, "bar"], [[".."], [".."], ["foo", 0xcd, "bar"]]);
        t!(s: "a/b/c", ["a", "b", "c"]);
        t!(s: "a/b/d", ["a", "b", "d"]);
        t!(s: "a/b/cd", ["a", "b", "cd"]);
        t!(s: "/a/b/c", ["a", "b", "c"]);
        t!(s: "a", ["a"]);
        t!(s: "/a", ["a"]);
        t!(s: "/", []);
        t!(s: ".", ["."]);
        t!(s: "..", [".."]);
        t!(s: "../..", ["..", ".."]);
        t!(s: "../../foo", ["..", "..", "foo"]);
    }

    #[test]
    fn test_str_components() {
        macro_rules! t(
            (v: [$($arg:expr),+], $exp:expr) => (
                {
                    let path = Path::new(b!($($arg),+));
                    let comps = path.str_components().collect::<Vec<Option<&str>>>();
                    let exp: &[Option<&str>] = $exp;
                    assert_eq!(comps.as_slice(), exp);
                    let comps = path.rev_str_components().collect::<Vec<Option<&str>>>();
                    let exp = exp.rev_iter().map(|&x|x).collect::<Vec<Option<&str>>>();
                    assert_eq!(comps, exp);
                }
            )
        )

        t!(v: ["a/b/c"], [Some("a"), Some("b"), Some("c")]);
        t!(v: ["/", 0xff, "/a/", 0x80], [None, Some("a"), None]);
        t!(v: ["../../foo", 0xcd, "bar"], [Some(".."), Some(".."), None]);
        // str_components is a wrapper around components, so no need to do
        // the full set of tests
    }
}

#[cfg(test)]
mod bench {
    extern crate test;
    use self::test::Bencher;
    use super::*;
    use prelude::*;

    #[bench]
    fn join_home_dir(b: &mut Bencher) {
        let posix_path = Path::new("/");
        b.iter(|| {
            posix_path.join("home");
        });
    }

    #[bench]
    fn join_abs_path_home_dir(b: &mut Bencher) {
        let posix_path = Path::new("/");
        b.iter(|| {
            posix_path.join("/home");
        });
    }

    #[bench]
    fn join_many_home_dir(b: &mut Bencher) {
        let posix_path = Path::new("/");
        b.iter(|| {
            posix_path.join_many(&["home"]);
        });
    }

    #[bench]
    fn join_many_abs_path_home_dir(b: &mut Bencher) {
        let posix_path = Path::new("/");
        b.iter(|| {
            posix_path.join_many(&["/home"]);
        });
    }

    #[bench]
    fn push_home_dir(b: &mut Bencher) {
        let mut posix_path = Path::new("/");
        b.iter(|| {
            posix_path.push("home");
        });
    }

    #[bench]
    fn push_abs_path_home_dir(b: &mut Bencher) {
        let mut posix_path = Path::new("/");
        b.iter(|| {
            posix_path.push("/home");
        });
    }

    #[bench]
    fn push_many_home_dir(b: &mut Bencher) {
        let mut posix_path = Path::new("/");
        b.iter(|| {
            posix_path.push_many(&["home"]);
        });
    }

    #[bench]
    fn push_many_abs_path_home_dir(b: &mut Bencher) {
        let mut posix_path = Path::new("/");
        b.iter(|| {
            posix_path.push_many(&["/home"]);
        });
    }

    #[bench]
    fn ends_with_path_home_dir(b: &mut Bencher) {
        let posix_home_path = Path::new("/home");
        b.iter(|| {
            posix_home_path.ends_with_path(&Path::new("home"));
        });
    }

    #[bench]
    fn ends_with_path_missmatch_jome_home(b: &mut Bencher) {
        let posix_home_path = Path::new("/home");
        b.iter(|| {
            posix_home_path.ends_with_path(&Path::new("jome"));
        });
    }

    #[bench]
    fn is_ancestor_of_path_with_10_dirs(b: &mut Bencher) {
        let path = Path::new("/home/1/2/3/4/5/6/7/8/9");
        let mut sub = path.clone();
        sub.pop();
        b.iter(|| {
            path.is_ancestor_of(&sub);
        });
    }

    #[bench]
    fn path_relative_from_forward(b: &mut Bencher) {
        let path = Path::new("/a/b/c");
        let mut other = path.clone();
        other.pop();
        b.iter(|| {
            path.path_relative_from(&other);
        });
    }

    #[bench]
    fn path_relative_from_same_level(b: &mut Bencher) {
        let path = Path::new("/a/b/c");
        let mut other = path.clone();
        other.pop();
        other.push("d");
        b.iter(|| {
            path.path_relative_from(&other);
        });
    }

    #[bench]
    fn path_relative_from_backward(b: &mut Bencher) {
        let path = Path::new("/a/b");
        let mut other = path.clone();
        other.push("c");
        b.iter(|| {
            path.path_relative_from(&other);
        });
    }
}
