// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::clone::{Clone, DeepClone};
use std::cmp::{TotalEq, Ord, TotalOrd, Equiv};
use std::cmp::Equal;
use std::container::{Container, Map, MutableMap};
use std::default::Default;
use std::send_str::{SendStr, SendStrOwned, SendStrStatic};
use std::str::Str;
use std::to_str::ToStr;
use std::hashmap::HashMap;
use std::option::Some;

pub fn main() {
    let mut map: HashMap<SendStr, uint> = HashMap::new();
    assert!(map.insert(SendStrStatic("foo"), 42));
    assert!(!map.insert(SendStrOwned(~"foo"), 42));
    assert!(!map.insert(SendStrStatic("foo"), 42));
    assert!(!map.insert(SendStrOwned(~"foo"), 42));

    assert!(!map.insert(SendStrStatic("foo"), 43));
    assert!(!map.insert(SendStrOwned(~"foo"), 44));
    assert!(!map.insert(SendStrStatic("foo"), 45));
    assert!(!map.insert(SendStrOwned(~"foo"), 46));

    let v = 46;

    assert_eq!(map.find(&SendStrOwned(~"foo")), Some(&v));
    assert_eq!(map.find(&SendStrStatic("foo")), Some(&v));

    let (a, b, c, d) = (50, 51, 52, 53);

    assert!(map.insert(SendStrStatic("abc"), a));
    assert!(map.insert(SendStrOwned(~"bcd"), b));
    assert!(map.insert(SendStrStatic("cde"), c));
    assert!(map.insert(SendStrOwned(~"def"), d));

    assert!(!map.insert(SendStrStatic("abc"), a));
    assert!(!map.insert(SendStrOwned(~"bcd"), b));
    assert!(!map.insert(SendStrStatic("cde"), c));
    assert!(!map.insert(SendStrOwned(~"def"), d));

    assert!(!map.insert(SendStrOwned(~"abc"), a));
    assert!(!map.insert(SendStrStatic("bcd"), b));
    assert!(!map.insert(SendStrOwned(~"cde"), c));
    assert!(!map.insert(SendStrStatic("def"), d));

    assert_eq!(map.find_equiv(&("abc")), Some(&a));
    assert_eq!(map.find_equiv(&("bcd")), Some(&b));
    assert_eq!(map.find_equiv(&("cde")), Some(&c));
    assert_eq!(map.find_equiv(&("def")), Some(&d));

    assert_eq!(map.find_equiv(&(~"abc")), Some(&a));
    assert_eq!(map.find_equiv(&(~"bcd")), Some(&b));
    assert_eq!(map.find_equiv(&(~"cde")), Some(&c));
    assert_eq!(map.find_equiv(&(~"def")), Some(&d));

    assert_eq!(map.find_equiv(&SendStrStatic("abc")), Some(&a));
    assert_eq!(map.find_equiv(&SendStrStatic("bcd")), Some(&b));
    assert_eq!(map.find_equiv(&SendStrStatic("cde")), Some(&c));
    assert_eq!(map.find_equiv(&SendStrStatic("def")), Some(&d));

    assert_eq!(map.find_equiv(&SendStrOwned(~"abc")), Some(&a));
    assert_eq!(map.find_equiv(&SendStrOwned(~"bcd")), Some(&b));
    assert_eq!(map.find_equiv(&SendStrOwned(~"cde")), Some(&c));
    assert_eq!(map.find_equiv(&SendStrOwned(~"def")), Some(&d));
}
