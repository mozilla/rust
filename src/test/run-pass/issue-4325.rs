// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct Node<'self, T> {
  val: T,
  next: Option<&'self Node<'self, T>>
}

impl<'self, T> Node<'self, T> {
  fn get(&self) -> &'self T {
    match self.next {
      Some(ref next) => next.get(),
      None => &self.val
    }
  }
}

pub fn main() {}
