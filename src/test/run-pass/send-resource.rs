// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::thread::Thread;
use std::sync::mpsc::channel;

struct test {
  f: int,
}

impl Drop for test {
    fn drop(&mut self) {}
}

fn test(f: int) -> test {
    test {
        f: f
    }
}

pub fn main() {
    let (tx, rx) = channel();

    let _t = Thread::spawn(move|| {
        let (tx2, rx2) = channel();
        tx.send(tx2).unwrap();

        let _r = rx2.recv().unwrap();
    });

    rx.recv().unwrap().send(test(42)).unwrap();
}
