// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Calculation and management of a Strict Version Hash for crates
//!
//! The SVH is used for incremental compilation to track when HIR
//! nodes have changed between compilations, and also to detect
//! mismatches where we have two versions of the same crate that were
//! compiled from distinct sources.

use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Svh {
    hash: u64,
}

impl Svh {
    /// Create a new `Svh` given the hash. If you actually want to
    /// compute the SVH from some HIR, you want the `calculate_svh`
    /// function found in `librustc_incremental`.
    pub fn new(hash: u64) -> Svh {
        Svh { hash: hash }
    }

    pub fn as_u64(&self) -> u64 {
        self.hash
    }

    pub fn to_string(&self) -> String {
        let hash = self.hash;
        return (0..64).step_by(4).map(|i| hex(hash >> i)).collect();

        fn hex(b: u64) -> char {
            let b = (b & 0xf) as u8;
            let b = match b {
                0 ... 9 => '0' as u8 + b,
                _ => 'a' as u8 + b - 10,
            };
            b as char
        }
    }
}

impl Hash for Svh {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        self.hash.to_le().hash(state);
    }
}

impl fmt::Display for Svh {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&self.to_string())
    }
}
