// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::sync::mpsc::{channel, Sender};
use std::thread::Thread;

fn start(tx: &Sender<int>, start: int, number_of_messages: int) {
    let mut i: int = 0;
    while i< number_of_messages { tx.send(start + i).unwrap(); i += 1; }
}

pub fn main() {
    println!("Check that we don't deadlock.");
    let (tx, rx) = channel();
    let _ = Thread::scoped(move|| { start(&tx, 0, 10) }).join();
    println!("Joined task");
}
