// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:lint_plugin_test.rs
// ignore-stage1

#![feature(plugin)]
#![forbid(test_lint)]

#[plugin] #[no_link]
extern crate lint_plugin_test;

fn lintme() { } //~ ERROR item is named 'lintme'

#[allow(test_lint)] //~ ERROR allow(test_lint) overruled by outer forbid(test_lint)
pub fn main() {
    lintme();
}
