// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct Str([u8]);

#[derive(Clone)]
struct CharSplits<'a, Sep> {
    string: &'a Str,
    sep: Sep,
    allow_trailing_empty: bool,
    only_ascii: bool,
    finished: bool,
}

fn clone(s: &Str) -> &Str {
    Clone::clone(&s)
}

fn main() {}
