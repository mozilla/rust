// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-tidy-linelength

impl<T> Option<T> { //~ERROR inherent implementations are not allowed for types not defined in the current module
    pub fn foo(&self) { }
}

fn main() { }
