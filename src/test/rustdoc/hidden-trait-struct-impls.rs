// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name = "foo"]

#[doc(hidden)]
pub trait Foo {}

trait Dark {}

pub trait Bam {}

pub struct Bar;

struct Hidden;

// @!has foo/struct.Bar.html '//*[@id="impl-Foo"]' 'impl Foo for Bar'
impl Foo for Bar {}
// @!has foo/struct.Bar.html '//*[@id="impl-Dark"]' 'impl Dark for Bar'
impl Dark for Bar {}
// @has foo/struct.Bar.html '//*[@id="impl-Bam"]' 'impl Bam for Bar'
// @has foo/trait.Bam.html '//*[@id="implementors-list"]' 'impl Bam for Bar'
impl Bam for Bar {}
// @!has foo/trait.Bam.html '//*[@id="implementors-list"]' 'impl Bam for Hidden'
impl Bam for Hidden {}
