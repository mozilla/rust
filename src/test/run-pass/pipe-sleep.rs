// xfail-fast

// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


extern mod std;
use std::timer::sleep;
use std::uv;
use pipes::recv;

proto! oneshot (
    waiting:send {
        signal -> !
    }
)

fn main() {
    use oneshot::client::*;

    let c = pipes::spawn_service(oneshot::init, |p| { recv(move p); });

    let iotask = &uv::global_loop::get();
    sleep(iotask, 500);
    
    signal(move c);
}
