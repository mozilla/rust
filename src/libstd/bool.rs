// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Operations on boolean values (`bool` type)
//!
//! A quick summary:
//!
//! ## Trait implementations for `bool`
//!
//! Implementations of the following traits:
//!
//! * `FromStr`
//! * `ToStr`
//! * `Not`
//! * `Ord`
//! * `TotalOrd`
//! * `Eq`
//! * `Default`
//! * `Zero`
//!
//! ## Various functions to compare `bool`s
//!
//! All of the standard comparison functions one would expect: `and`, `eq`, `or`,
//! and more.
//!
//! Also, a few conversion functions: `to_bit` and `to_str`.

use option::{None, Option, Some};
use from_str::FromStr;
use to_str::ToStr;
use num::FromPrimitive;

#[cfg(not(test))] use cmp::{Eq, Ord, TotalOrd, Ordering};
#[cfg(not(test))] use ops::{Not, BitAnd, BitOr, BitXor};
#[cfg(not(test))] use default::Default;

/////////////////////////////////////////////////////////////////////////////
// Freestanding functions
/////////////////////////////////////////////////////////////////////////////

/// Iterates over all truth values, passing them to the given block.
///
/// There are no guarantees about the order values will be given.
///
/// # Examples
///
/// ```
/// std::bool::all_values(|x: bool| {
///     println!("{}", x);
/// })
/// ```
#[inline]
pub fn all_values(blk: |v: bool|) {
    blk(true);
    blk(false);
}

/////////////////////////////////////////////////////////////////////////////
// Methods on `bool`
/////////////////////////////////////////////////////////////////////////////

/// Extension methods on a `bool`
pub trait Bool {
    /// Conjunction of two boolean values.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(true.and(true), true);
    /// fail_unless_eq!(true.and(false), false);
    /// fail_unless_eq!(false.and(true), false);
    /// fail_unless_eq!(false.and(false), false);
    /// ```
    fn and(self, b: bool) -> bool;

    /// Disjunction of two boolean values.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(true.or(true), true);
    /// fail_unless_eq!(true.or(false), true);
    /// fail_unless_eq!(false.or(true), true);
    /// fail_unless_eq!(false.or(false), false);
    /// ```
    fn or(self, b: bool) -> bool;

    /// An 'exclusive or' of two boolean values.
    ///
    /// 'exclusive or' is identical to `or(and(a, not(b)), and(not(a), b))`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(true.xor(true), false);
    /// fail_unless_eq!(true.xor(false), true);
    /// fail_unless_eq!(false.xor(true), true);
    /// fail_unless_eq!(false.xor(false), false);
    /// ```
    fn xor(self, b: bool) -> bool;

    /// Implication between two boolean values.
    ///
    /// Implication is often phrased as 'if a then b.'
    ///
    /// 'if a then b' is equivalent to `!a || b`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(true.implies(true), true);
    /// fail_unless_eq!(true.implies(false), false);
    /// fail_unless_eq!(false.implies(true), true);
    /// fail_unless_eq!(false.implies(false), true);
    /// ```
    fn implies(self, b: bool) -> bool;

    /// Convert a `bool` to a `u8`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(true.to_bit::<u8>(), 1u8);
    /// fail_unless_eq!(false.to_bit::<u8>(), 0u8);
    /// ```
    fn to_bit<N: FromPrimitive>(self) -> N;
}

impl Bool for bool {
    #[inline]
    fn and(self, b: bool) -> bool { self && b }

    #[inline]
    fn or(self, b: bool) -> bool { self || b }

    #[inline]
    fn xor(self, b: bool) -> bool { self ^ b }

    #[inline]
    fn implies(self, b: bool) -> bool { !self || b }

    #[inline]
    fn to_bit<N: FromPrimitive>(self) -> N {
        if self { FromPrimitive::from_u8(1).unwrap() }
        else    { FromPrimitive::from_u8(0).unwrap() }
    }
}

/////////////////////////////////////////////////////////////////////////////
// Trait impls on `bool`
/////////////////////////////////////////////////////////////////////////////

impl FromStr for bool {
    /// Parse a `bool` from a string.
    ///
    /// Yields an `Option<bool>`, because `s` may or may not actually be parseable.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(from_str::<bool>("true"), Some(true));
    /// fail_unless_eq!(from_str::<bool>("false"), Some(false));
    /// fail_unless_eq!(from_str::<bool>("not even a boolean"), None);
    /// ```
    #[inline]
    fn from_str(s: &str) -> Option<bool> {
        match s {
            "true"  => Some(true),
            "false" => Some(false),
            _       => None,
        }
    }
}

impl ToStr for bool {
    /// Convert a `bool` to a string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(true.to_str(), ~"true");
    /// fail_unless_eq!(false.to_str(), ~"false");
    /// ```
    #[inline]
    fn to_str(&self) -> ~str {
        if *self { ~"true" } else { ~"false" }
    }
}

#[cfg(not(test))]
impl Not<bool> for bool {
    /// The logical complement of a boolean value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(!true, false);
    /// fail_unless_eq!(!false, true);
    /// ```
    #[inline]
    fn not(&self) -> bool { !*self }
}

#[cfg(not(test))]
impl BitAnd<bool, bool> for bool {
    /// Conjunction of two boolean values.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(false.bitand(&false), false);
    /// fail_unless_eq!(true.bitand(&false), false);
    /// fail_unless_eq!(false.bitand(&true), false);
    /// fail_unless_eq!(true.bitand(&true), true);
    ///
    /// fail_unless_eq!(false & false, false);
    /// fail_unless_eq!(true & false, false);
    /// fail_unless_eq!(false & true, false);
    /// fail_unless_eq!(true & true, true);
    /// ```
    #[inline]
    fn bitand(&self, b: &bool) -> bool { *self & *b }
}

#[cfg(not(test))]
impl BitOr<bool, bool> for bool {
    /// Disjunction of two boolean values.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(false.bitor(&false), false);
    /// fail_unless_eq!(true.bitor(&false), true);
    /// fail_unless_eq!(false.bitor(&true), true);
    /// fail_unless_eq!(true.bitor(&true), true);
    ///
    /// fail_unless_eq!(false | false, false);
    /// fail_unless_eq!(true | false, true);
    /// fail_unless_eq!(false | true, true);
    /// fail_unless_eq!(true | true, true);
    /// ```
    #[inline]
    fn bitor(&self, b: &bool) -> bool { *self | *b }
}

#[cfg(not(test))]
impl BitXor<bool, bool> for bool {
    /// An 'exclusive or' of two boolean values.
    ///
    /// 'exclusive or' is identical to `or(and(a, not(b)), and(not(a), b))`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// fail_unless_eq!(false.bitxor(&false), false);
    /// fail_unless_eq!(true.bitxor(&false), true);
    /// fail_unless_eq!(false.bitxor(&true), true);
    /// fail_unless_eq!(true.bitxor(&true), false);
    ///
    /// fail_unless_eq!(false ^ false, false);
    /// fail_unless_eq!(true ^ false, true);
    /// fail_unless_eq!(false ^ true, true);
    /// fail_unless_eq!(true ^ true, false);
    /// ```
    #[inline]
    fn bitxor(&self, b: &bool) -> bool { *self ^ *b }
}

#[cfg(not(test))]
impl Ord for bool {
    #[inline]
    fn lt(&self, other: &bool) -> bool { self.to_bit::<u8>() < other.to_bit() }
}

#[cfg(not(test))]
impl TotalOrd for bool {
    #[inline]
    fn cmp(&self, other: &bool) -> Ordering { self.to_bit::<u8>().cmp(&other.to_bit()) }
}

/// Equality between two boolean values.
///
/// Two booleans are equal if they have the same value.
///
/// # Examples
///
/// ```rust
/// fail_unless_eq!(false.eq(&true), false);
/// fail_unless_eq!(false == false, true);
/// fail_unless_eq!(false != true, true);
/// fail_unless_eq!(false.ne(&false), false);
/// ```
#[cfg(not(test))]
impl Eq for bool {
    #[inline]
    fn eq(&self, other: &bool) -> bool { (*self) == (*other) }
}

#[cfg(not(test))]
impl Default for bool {
    fn default() -> bool { false }
}

#[cfg(test)]
mod tests {
    use prelude::*;
    use super::all_values;

    #[test]
    fn test_bool() {
        fail_unless_eq!(false.eq(&true), false);
        fail_unless_eq!(false == false, true);
        fail_unless_eq!(false != true, true);
        fail_unless_eq!(false.ne(&false), false);

        fail_unless_eq!(false.bitand(&false), false);
        fail_unless_eq!(true.bitand(&false), false);
        fail_unless_eq!(false.bitand(&true), false);
        fail_unless_eq!(true.bitand(&true), true);

        fail_unless_eq!(false & false, false);
        fail_unless_eq!(true & false, false);
        fail_unless_eq!(false & true, false);
        fail_unless_eq!(true & true, true);

        fail_unless_eq!(false.bitor(&false), false);
        fail_unless_eq!(true.bitor(&false), true);
        fail_unless_eq!(false.bitor(&true), true);
        fail_unless_eq!(true.bitor(&true), true);

        fail_unless_eq!(false | false, false);
        fail_unless_eq!(true | false, true);
        fail_unless_eq!(false | true, true);
        fail_unless_eq!(true | true, true);

        fail_unless_eq!(false.bitxor(&false), false);
        fail_unless_eq!(true.bitxor(&false), true);
        fail_unless_eq!(false.bitxor(&true), true);
        fail_unless_eq!(true.bitxor(&true), false);

        fail_unless_eq!(false ^ false, false);
        fail_unless_eq!(true ^ false, true);
        fail_unless_eq!(false ^ true, true);
        fail_unless_eq!(true ^ true, false);

        fail_unless_eq!(!true, false);
        fail_unless_eq!(!false, true);

        fail_unless_eq!(true.to_str(), ~"true");
        fail_unless_eq!(false.to_str(), ~"false");

        fail_unless_eq!(from_str::<bool>("true"), Some(true));
        fail_unless_eq!(from_str::<bool>("false"), Some(false));
        fail_unless_eq!(from_str::<bool>("not even a boolean"), None);

        fail_unless_eq!(true.and(true), true);
        fail_unless_eq!(true.and(false), false);
        fail_unless_eq!(false.and(true), false);
        fail_unless_eq!(false.and(false), false);

        fail_unless_eq!(true.or(true), true);
        fail_unless_eq!(true.or(false), true);
        fail_unless_eq!(false.or(true), true);
        fail_unless_eq!(false.or(false), false);

        fail_unless_eq!(true.xor(true), false);
        fail_unless_eq!(true.xor(false), true);
        fail_unless_eq!(false.xor(true), true);
        fail_unless_eq!(false.xor(false), false);

        fail_unless_eq!(true.implies(true), true);
        fail_unless_eq!(true.implies(false), false);
        fail_unless_eq!(false.implies(true), true);
        fail_unless_eq!(false.implies(false), true);

        fail_unless_eq!(true.to_bit::<u8>(), 1u8);
        fail_unless_eq!(false.to_bit::<u8>(), 0u8);
    }

    #[test]
    fn test_bool_from_str() {
        all_values(|v| {
            fail_unless!(Some(v) == FromStr::from_str(v.to_str()))
        });
    }

    #[test]
    fn test_bool_to_str() {
        fail_unless_eq!(false.to_str(), ~"false");
        fail_unless_eq!(true.to_str(), ~"true");
    }

    #[test]
    fn test_bool_to_bit() {
        all_values(|v| {
            fail_unless_eq!(v.to_bit::<u8>(), if v { 1u8 } else { 0u8 });
            fail_unless_eq!(v.to_bit::<uint>(), if v { 1u } else { 0u });
            fail_unless_eq!(v.to_bit::<int>(), if v { 1i } else { 0i });
        });
    }

    #[test]
    fn test_bool_ord() {
        fail_unless!(true > false);
        fail_unless!(!(false > true));

        fail_unless!(false < true);
        fail_unless!(!(true < false));

        fail_unless!(false <= false);
        fail_unless!(false >= false);
        fail_unless!(true <= true);
        fail_unless!(true >= true);

        fail_unless!(false <= true);
        fail_unless!(!(false >= true));
        fail_unless!(true >= false);
        fail_unless!(!(true <= false));
    }

    #[test]
    fn test_bool_totalord() {
        fail_unless_eq!(true.cmp(&true), Equal);
        fail_unless_eq!(false.cmp(&false), Equal);
        fail_unless_eq!(true.cmp(&false), Greater);
        fail_unless_eq!(false.cmp(&true), Less);
    }
}
