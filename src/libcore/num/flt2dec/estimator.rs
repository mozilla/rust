// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The exponent estimator.

/// Finds `k_0` such that `10^(k_0-1) < mant * 2^exp <= 10^(k_0+1)`.
///
/// This is used to approximate `k = ceil(log_10 (mant * 2^exp))`;
/// the true `k` is either `k_0` or `k_0+1`.
#[doc(hidden)]
pub fn estimate_scaling_factor(mant: u64, exp: i16) -> i16 {
    // 2^(nbits-1) < mant <= 2^nbits if mant > 0
    let nbits = 64 - (mant - 1).leading_zeros() as i64;
    // 1292913986 = floor(2^32 * log_10 2)
    // therefore this always underestimates (or is exact), but not much.
    (((nbits + exp as i64) * 1292913986) >> 32) as i16
}

