// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:plugin_args.rs
// ignore-stage1

#![feature(plugin)]

#[no_link]
#[plugin="foobar"]
extern crate plugin_args;

fn main() {
    assert_eq!(plugin_args!(), "#[plugin = \"foobar\"]");
}
