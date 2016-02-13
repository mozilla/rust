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

// @has issue_29449/struct.Foo.html
pub struct Foo;

impl Foo {
    // @has - '//*[@id="examples"]//a' 'Examples'
    // @has - '//*[@id="panics"]//a' 'Panics'
    /// # Examples
    /// # Panics
    pub fn bar() {}

    // @has - '//*[@id="examples-1"]//a' 'Examples'
    /// # Examples
    pub fn bar_1() {}

    // @has - '//*[@id="examples-2"]//a' 'Examples'
    // @has - '//*[@id="panics-1"]//a' 'Panics'
    /// # Examples
    /// # Panics
    pub fn bar_2() {}
}

/* !search-index
{
    "issue_29449": {
        "issue_29449::Foo": [
            "Struct"
        ],
        "issue_29449::Foo<Struct>::bar": [
            "Method(foo)",
            "# Examples\n# Panics"
        ],
        "issue_29449::Foo<Struct>::bar_1": [
            "Method(foo)",
            "# Examples"
        ],
        "issue_29449::Foo<Struct>::bar_2": [
            "Method(foo)",
            "# Examples\n# Panics"
        ]
    }
}
*/
