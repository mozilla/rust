// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[macro_escape];
#[doc(hidden)];

macro_rules! rterrln (
    ($($arg:tt)*) => ( {
        format_args!(::rt::util::dumb_println, $($arg)*)
    } )
)

// Some basic logging. Enabled by passing `--cfg rtdebug` to the libstd build.
macro_rules! rtdebug (
    ($($arg:tt)*) => ( {
        if cfg!(rtdebug) {
            rterrln!($($arg)*)
        }
    })
)

macro_rules! rtassert (
    ( $arg:expr ) => ( {
        if ::rt::util::ENFORCE_SANITY {
            if !$arg {
                rtabort!("assertion failed: {}", stringify!($arg));
            }
        }
    } )
)


macro_rules! rtabort(
    ($($msg:tt)*) => ( {
        ::rt::util::abort(format!($($msg)*));
    } )
)
