// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unknown_features)]
#![feature(box_syntax)]

use std::sync::mpsc::channel;

pub fn main() {
    let (tx, rx) = channel();
    tx.send(box 100i).unwrap();
    let v = rx.recv().unwrap();
    assert_eq!(v, box 100i);
}
