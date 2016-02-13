// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// check-search-index

#![feature(lang_items)]
#![no_std]

pub mod str {
    #![doc(primitive = "str")]

    #[lang = "str"]
    impl str {
        // @has search-index.js foo
        pub fn foo(&self) {}
    }
}

/* !search-index
{
    "issue_23511": {
        "issue_23511::str": [
            "Primitive"
        ],
        "issue_23511::str<Primitive>::foo": [
            "Method(str)"
        ]
    }
}
*/
