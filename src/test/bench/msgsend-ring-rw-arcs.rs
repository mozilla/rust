// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// This test creates a bunch of tasks that simultaneously send to each
// other in a ring. The messages should all be basically
// independent.
// This is like msgsend-ring-pipes but adapted to use ARCs.

// This also serves as a pipes test, because ARCs are implemented with pipes.

extern mod extra;

use extra::sync::{RWArc, WaitQueue};
use extra::future;
use extra::time;
use std::cell::Cell;
use std::io;
use std::os;
use std::uint;


#[deriving(Clone)]
struct Pipe {
    rwarc: RWArc<~[uint]>,
    condition: WaitQueue
}

fn send(p: &mut Pipe, msg: uint) {
    do p.rwarc.write |queue| {
        queue.push(msg)
    }
    p.condition.signal();
}

fn recv(p: &mut Pipe) -> uint {
    loop {
        let maybe_pop_count = do p.rwarc.write |queue| {
            if queue.is_empty() { None } else {
                Some(queue.pop())
            }
        };

        match maybe_pop_count {
            None => p.condition.wait(),
            Some(pop_count) => return pop_count
        }
    }
}

fn init() -> (Pipe, Pipe) {
    let x = Pipe { rwarc: RWArc::new(~[]), condition: WaitQueue::new() };
    (x.clone(), x)
}

fn thread_ring(i: uint, count: uint, num_chan: Pipe, num_port: Pipe) {
    let mut num_chan = Some(num_chan);
    let mut num_port = Some(num_port);
    // Send/Receive lots of messages.
    for uint::range(0u, count) |j| {
        //error!("task %?, iter %?", i, j);

        let mut num_chan2 = num_chan.take_unwrap();
        let mut num_port2 = num_port.take_unwrap();
        send(&mut num_chan2, i * j);

        num_chan = Some(num_chan2);
        let _n = recv(&mut num_port2);
        //log(error, _n);
        num_port = Some(num_port2);
    };
}

fn main() {
    let args = os::args();
    let args = if os::getenv("RUST_BENCH").is_some() {
        ~[~"", ~"100", ~"10000"]
    } else if args.len() <= 1u {
        ~[~"", ~"10", ~"100"]
    } else {
        args.clone()
    };

    let num_tasks = uint::from_str(args[1]).get();
    let msg_per_task = uint::from_str(args[2]).get();

    let (num_chan, num_port) = init();
    let num_chan = Cell::new(num_chan);

    let start = time::precise_time_s();

    // create the ring
    let mut futures = ~[];

    for uint::range(1u, num_tasks) |i| {
        //error!("spawning %?", i);
        let (new_chan, num_port) = init();
        let num_chan2 = Cell::new(num_chan.take());
        let num_port = Cell::new(num_port);
        let new_future = do future::spawn {
            let num_chan = num_chan2.take();
            let num_port1 = num_port.take();
            thread_ring(i, msg_per_task, num_chan, num_port1)
        };
        futures.push(new_future);
        num_chan.put_back(new_chan);
    };

    // do our iteration
    thread_ring(0, msg_per_task, num_chan.take(), num_port);

    // synchronize
    for futures.mut_iter().advance |f| {
        let _ = f.get();
    }

    let stop = time::precise_time_s();

    // all done, report stats.
    let num_msgs = num_tasks * msg_per_task;
    let elapsed = (stop - start);
    let rate = (num_msgs as float) / elapsed;

    io::println(fmt!("Sent %? messages in %? seconds",
                     num_msgs, elapsed));
    io::println(fmt!("  %? messages / second", rate));
    io::println(fmt!("  %? μs / message", 1000000. / rate));
}
