// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![sanitizer_runtime]
#![feature(alloc_system)]
#![feature(allocator_api)]
#![feature(global_allocator)]
#![feature(sanitizer_runtime)]
#![feature(staged_api)]
#![no_std]
#![unstable(feature = "sanitizer_runtime_lib",
            reason = "internal implementation detail of sanitizers",
            issue = "0")]

extern crate alloc_system;

#[cfg(not(stage0))]
use alloc_system::System;

// The GlobalAllocator trait has changed since stage0
#[cfg(not(stage0))]
#[global_allocator]
static ALLOC: System = System;
