// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use extra::url;
use version::{try_getting_version, try_getting_local_version,
              Version, NoVersion, split_version};
use std::hash::Streaming;
use std::hash;
use messages::error;

/// Path-fragment identifier of a package such as
/// 'github.com/graydon/test'; path must be a relative
/// path with >=1 component.
#[deriving(Clone)]
pub struct PkgId {
    /// This is a path, on the local filesystem, referring to where the
    /// files for this package live. For example:
    /// github.com/mozilla/quux-whatever (it's assumed that if we're
    /// working with a package ID of this form, rustpkg has already cloned
    /// the sources into a local directory in the RUST_PATH).
    path: Path,
    /// Short name. This is the path's filestem, but we store it
    /// redundantly so as to not call get() everywhere (filestem() returns an
    /// option)
    /// The short name does not need to be a valid Rust identifier.
    /// Users can write: `extern mod foo = "...";` to get around the issue
    /// of package IDs whose short names aren't valid Rust identifiers.
    short_name: ~str,
    /// The requested package version.
    version: Version
}

impl Eq for PkgId {
    fn eq(&self, other: &PkgId) -> bool {
        self.path == other.path && self.version == other.version
    }
}

/// Parses `s` as a URL and, if the parse was successful, returns the
/// portion of the URL without the scheme
/// for example: git://github.com/catamorphism/foo => github.com/catamorphism/foo
fn drop_url_scheme<'a>(s: &'a str) -> Option<~str> {
    match url::get_scheme(s) {
        Ok((_, rest)) => {
            // get_scheme leaves in the leading //
            let len = rest.len();
            let path_to_use = if len > 2 {
                let no_slashes = rest.slice(2, len);
                // Don't display the .git extension, since it's not part of the actual
                // package ID
                if no_slashes.ends_with(".git") {
                    no_slashes.slice(0, len - 6).to_owned()
                }
                else {
                    no_slashes.to_owned()
                }
            } else {
                rest
            };
            Some(path_to_use)
        }
        Err(_)        => None
    }
}

// Fails if this is not a legal package ID,
// after printing a hint
fn ensure_legal_package_id(s: &str) {
    // url::get_path checks that the string contains characters
    // that are legal in a URL ('#' is legal, so we're good there)
    let maybe_intended_path = drop_url_scheme(s);
    let legal = maybe_intended_path.is_none()
        && url::get_path(s, false).is_ok();
    if !legal {
        for maybe_package_id in maybe_intended_path.iter() {
            error(format!("rustpkg operates on package IDs; did you mean to write \
                          `{}` instead of `{}`?",
                  *maybe_package_id,
                  s));
        }
        fail!("Can't parse {} as a package ID", s);
    }
}

impl PkgId {
    pub fn new(s: &str) -> PkgId {
        use conditions::bad_pkg_id::cond;

        // Make sure the path is a legal package ID -- it might not even
        // be a legal path, so we do this first
        ensure_legal_package_id(s);

        let mut given_version = None;

        // Did the user request a specific version?
        let s = match split_version(s) {
            Some((path, v)) => {
                given_version = Some(v);
                path
            }
            None => {
                s
            }
        };

        let path = Path::init(s);
        if !path.is_relative() {
            return cond.raise((path, ~"absolute pkgid"));
        }
        if path.filename().is_none() {
            return cond.raise((path, ~"0-length pkgid"));
        }
        let short_name = path.filestem_str().expect(format!("Strange path! {}", s));

        let version = match given_version {
            Some(v) => v,
            None => match try_getting_local_version(&path) {
                Some(v) => v,
                None => match try_getting_version(&path) {
                    Some(v) => v,
                    None => NoVersion
                }
            }
        };

        PkgId {
            path: path.clone(),
            short_name: short_name.to_owned(),
            version: version
        }
    }

    pub fn hash(&self) -> ~str {
        // FIXME (#9639): hash should take a &[u8] so we can hash the real path
        self.path.display().with_str(|s| {
            let vers = self.version.to_str();
            format!("{}-{}-{}", s, hash(s + vers), vers)
        })
    }

    pub fn short_name_with_version(&self) -> ~str {
        format!("{}{}", self.short_name, self.version.to_str())
    }

    /// True if the ID has multiple components
    pub fn is_complex(&self) -> bool {
        self.short_name.as_bytes() != self.path.as_vec()
    }

    pub fn prefixes(&self) -> Prefixes {
        prefixes(&self.path)
    }

    // This is the workcache function name for the *installed*
    // binaries for this package (as opposed to the built ones,
    // which are per-crate).
    pub fn install_tag(&self) -> ~str {
        format!("install({})", self.to_str())
    }
}

pub fn prefixes(p: &Path) -> Prefixes {
    Prefixes {
        components: p.str_components().map(|x|x.unwrap().to_owned()).to_owned_vec(),
        remaining: ~[]
    }
}

struct Prefixes {
    priv components: ~[~str],
    priv remaining: ~[~str]
}

impl Iterator<(Path, Path)> for Prefixes {
    #[inline]
    fn next(&mut self) -> Option<(Path, Path)> {
        if self.components.len() <= 1 {
            None
        }
        else {
            let last = self.components.pop();
            self.remaining.unshift(last);
            // converting to str and then back is a little unfortunate
            Some((Path::init(self.components.connect("/")),
                  Path::init(self.remaining.connect("/"))))
        }
    }
}

impl ToStr for PkgId {
    fn to_str(&self) -> ~str {
        // should probably use the filestem and not the whole path
        format!("{}-{}", self.path.as_str().unwrap(), self.version.to_str())
    }
}


pub fn write<W: Writer>(writer: &mut W, string: &str) {
    writer.write(string.as_bytes());
}

pub fn hash(data: ~str) -> ~str {
    let hasher = &mut hash::default_state();
    write(hasher, data);
    hasher.result_str()
}

