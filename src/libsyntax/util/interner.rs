// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// An "interner" is a data structure that associates values with uint tags and
// allows bidirectional lookup; i.e. given a value, one can easily find the
// type, and vice versa.

use core::prelude::*;

use hashmap::linear::LinearMap;
use dvec::DVec;

pub struct Interner<T> {
    priv map: @mut LinearMap<T, uint>,
    priv vect: DVec<T>,
}

// when traits can extend traits, we should extend index<uint,T> to get []
pub impl<T:Eq + IterBytes + Hash + Const + Copy> Interner<T> {
    static fn new() -> Interner<T> {
        Interner {
            map: @mut LinearMap::new(),
            vect: DVec(),
        }
    }

    static fn prefill(init: &[T]) -> Interner<T> {
        let rv = Interner::new();
        for init.each() |v| { rv.intern(*v); }
        rv
    }

    fn intern(&self, val: T) -> uint {
        match self.map.find(&val) {
            Some(&idx) => return idx,
            None => (),
        }

        let new_idx = self.vect.len();
        self.map.insert(val, new_idx);
        self.vect.push(val);
        new_idx
    }

    fn gensym(&self, val: T) -> uint {
        let new_idx = self.vect.len();
        // leave out of .map to avoid colliding
        self.vect.push(val);
        new_idx
    }

    // this isn't "pure" in the traditional sense, because it can go from
    // failing to returning a value as items are interned. But for typestate,
    // where we first check a pred and then rely on it, ceasing to fail is ok.
    pure fn get(&self, idx: uint) -> T { self.vect.get_elt(idx) }

    fn len(&self) -> uint { self.vect.len() }
}

#[test]
#[should_fail]
pub fn i1 () {
    let i : Interner<@~str> = Interner::new();
    i.get(13);
}

#[test]
pub fn i2 () {
    let i : Interner<@~str> = Interner::new();
    // first one is zero:
    assert i.intern (@~"dog") == 0;
    // re-use gets the same entry:
    assert i.intern (@~"dog") == 0;
    // different string gets a different #:
    assert i.intern (@~"cat") == 1;
    assert i.intern (@~"cat") == 1;
    // dog is still at zero
    assert i.intern (@~"dog") == 0;
    // gensym gets 3
    assert i.gensym (@~"zebra" ) == 2;
    // gensym of same string gets new number :
    assert i.gensym (@~"zebra" ) == 3;
    // gensym of *existing* string gets new number:
    assert i.gensym (@~"dog") == 4;
    assert i.get(0) == @~"dog";
    assert i.get(1) == @~"cat";
    assert i.get(2) == @~"zebra";
    assert i.get(3) == @~"zebra";
    assert i.get(4) == @~"dog";
}

#[test]
pub fn i3 () {
    let i : Interner<@~str> = Interner::prefill([@~"Alan",@~"Bob",@~"Carol"]);
    assert i.get(0) == @~"Alan";
    assert i.get(1) == @~"Bob";
    assert i.get(2) == @~"Carol";
    assert i.intern(@~"Bob") == 1;
}
