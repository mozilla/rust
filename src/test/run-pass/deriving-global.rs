// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(old_orphan_check)]

extern crate serialize;
extern crate rand;

mod submod {
    // if any of these are implemented without global calls for any
    // function calls, then being in a submodule will (correctly)
    // cause errors about unrecognised module `std` (or `extra`)
    #[derive(PartialEq, PartialOrd, Eq, Ord,
               Hash,
               Clone,
               Show, Rand,
               Encodable, Decodable)]
    enum A { A1(uint), A2(int) }

    #[derive(PartialEq, PartialOrd, Eq, Ord,
               Hash,
               Clone,
               Show, Rand,
               Encodable, Decodable)]
    struct B { x: uint, y: int }

    #[derive(PartialEq, PartialOrd, Eq, Ord,
               Hash,
               Clone,
               Show, Rand,
               Encodable, Decodable)]
    struct C(uint, int);

}

pub fn main() {}
