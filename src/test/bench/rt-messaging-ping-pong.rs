// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern mod extra;

use std::os;
use std::uint;
use std::rt::test::spawntask_later;


// This is a simple bench that creates M pairs of of tasks. These
// tasks ping-pong back and forth over a pair of streams. This is a
// cannonical message-passing benchmark as it heavily strains message
// passing and almost nothing else.

fn ping_pong_bench(n: uint, m: uint) {

    // Create pairs of tasks that pingpong back and forth.
    fn run_pair(n: uint) {
            // Create a stream A->B
            let (pa,ca) = stream::<()>();
            // Create a stream B->A
            let (pb,cb) = stream::<()>();

            let pa = Mut::new_some(pa);
            let ca = Mut::new_some(ca);
            let pb = Mut::new_some(pb);
            let cb = Mut::new_some(cb);

        do spawntask_later() || {
            let chan = ca.take_unwrap();
            let port = pb.take_unwrap();
            do n.times {
                chan.send(());
                port.recv();
            }
        }

        do spawntask_later() || {
            let chan = cb.take_unwrap();
            let port = pa.take_unwrap();
            do n.times {
                port.recv();
                chan.send(());
            }
        }
    }

    do m.times {
        run_pair(n)
    }

}



fn main() {

    let args = os::args();
    let n = if args.len() == 3 {
        from_str::<uint>(args[1]).unwrap()
    } else {
        10000
    };

    let m = if args.len() == 3 {
        from_str::<uint>(args[2]).unwrap()
    } else {
        4
    };

    ping_pong_bench(n, m);

}
