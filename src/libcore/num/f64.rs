// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Operations and constants for 64-bits floats (`f64` type)

#![doc(primitive = "f64")]

use intrinsics;
use mem;
use num::{FPNormal, FPCategory, FPZero, FPSubnormal, FPInfinite, FPNaN};
use num::Float;
use option::Option;

// FIXME(#5527): These constants should be deprecated once associated
// constants are implemented in favour of referencing the respective
// members of `Bounded` and `Float`.

pub static RADIX: uint = 2u;

pub static MANTISSA_DIGITS: uint = 53u;
pub static DIGITS: uint = 15u;

pub static EPSILON: f64 = 2.2204460492503131e-16_f64;

/// Smallest finite f64 value
pub static MIN_VALUE: f64 = -1.7976931348623157e+308_f64;
/// Smallest positive, normalized f64 value
pub static MIN_POS_VALUE: f64 = 2.2250738585072014e-308_f64;
/// Largest finite f64 value
pub static MAX_VALUE: f64 = 1.7976931348623157e+308_f64;

pub static MIN_EXP: int = -1021;
pub static MAX_EXP: int = 1024;

pub static MIN_10_EXP: int = -307;
pub static MAX_10_EXP: int = 308;

pub static NAN: f64 = 0.0_f64/0.0_f64;
pub static INFINITY: f64 = 1.0_f64/0.0_f64;
pub static NEG_INFINITY: f64 = -1.0_f64/0.0_f64;

/// Various useful constants.
pub mod consts {
    // FIXME: replace with mathematical constants from cmath.

    // FIXME(#5527): These constants should be deprecated once associated
    // constants are implemented in favour of referencing the respective members
    // of `Float`.

    /// Archimedes' constant
    pub static TAU: f64 = 6.28318530717958647692528676655900576_f64;
    /// pi * 2.0 = tau
    pub static PI_2: f64 = TAU;

    /// tau / 2.0 = pi
    pub static FRAC_TAU_2: f64 = 3.14159265358979323846264338327950288_f64;
    /// pi = tau / 2.0
    pub static PI: f64 = FRAC_TAU_2;

    /// tau / 3.0 = 2.0 * pi / 3.0
    pub static FRAC_TAU_3: f64 = 2.094395102393195492308428922186335256_f64;
    /// 2.0 * pi / 3.0 = tau / 3.0
    pub static FRAC_2PI_3: f64 = FRAC_TAU_3;

    /// tau / 4.0 = pi / 2.0
    pub static FRAC_TAU_4: f64 = 1.57079632679489661923132169163975144_f64;
    /// pi / 2.0 = tau / 4.0
    pub static FRAC_PI_2: f64 = FRAC_TAU_4;

    /// tau / 6.0 = pi / 3.0
    pub static FRAC_TAU_6: f64 = 1.04719755119659774615421446109316763_f64;
    /// pi / 3.0 = tau / 6.0
    pub static FRAC_PI_3: f64 = FRAC_TAU_6;

    /// tau / 8.0 = pi / 4.0
    pub static FRAC_TAU_8: f64 = 0.785398163397448309615660845819875721_f64;
    /// pi / 4.0 = tau / 8.0
    pub static FRAC_PI_4: f64 = FRAC_TAU_8;

    /// tau / 12.0 = pi / 6.0
    pub static FRAC_TAU_12: f64 = 0.52359877559829887307710723054658381_f64;
    /// pi / 6.0 = tau / 12.0
    pub static FRAC_PI_6: f64 = FRAC_TAU_12;

    /// tau / 16.0 = pi / 8.0
    pub static FRAC_TAU_16: f64 = 0.39269908169872415480783042290993786_f64;
    /// pi / 8.0 = tau / 16.0
    pub static FRAC_PI_8: f64 = FRAC_TAU_16;

    /// 1.0 / tau = 1.0 / (pi * 2.0)
    pub static FRAC_1_TAU: f64 = 0.159154943091895335768883763372514362_f64;
    /// 1.0 / (pi * 2.0) = 1.0 / tau
    pub static FRAC_1_2PI: f64 = FRAC_1_TAU;

    /// 2.0 / tau = 1.0 / pi
    pub static FRAC_2_TAU: f64 = 0.318309886183790671537767526745028724_f64;
    /// 1.0 / pi = 2.0 / tau
    pub static FRAC_1_PI: f64 = FRAC_2_TAU;

    /// 4.0 / tau = 2.0 / pi
    pub static FRAC_4_TAU: f64 = 0.636619772367581343075535053490057448_f64;
    /// 2.0 / pi = 4.0 / tau
    pub static FRAC_2_PI: f64 = FRAC_4_TAU;

    /// 2.0 / sqrt(pi)
    pub static FRAC_2_SQRTPI: f64 = 1.12837916709551257389615890312154517_f64;

    /// sqrt(2.0)
    pub static SQRT2: f64 = 1.41421356237309504880168872420969808_f64;

    /// 1.0 / sqrt(2.0)
    pub static FRAC_1_SQRT2: f64 = 0.707106781186547524400844362104849039_f64;

    /// Euler's number
    pub static E: f64 = 2.71828182845904523536028747135266250_f64;

    /// log2(e)
    pub static LOG2_E: f64 = 1.44269504088896340735992468100189214_f64;

    /// log10(e)
    pub static LOG10_E: f64 = 0.434294481903251827651128918916605082_f64;

    /// ln(2.0)
    pub static LN_2: f64 = 0.693147180559945309417232121458176568_f64;

    /// ln(10.0)
    pub static LN_10: f64 = 2.30258509299404568401799145468436421_f64;
}

impl Float for f64 {
    #[inline]
    fn nan() -> f64 { NAN }

    #[inline]
    fn infinity() -> f64 { INFINITY }

    #[inline]
    fn neg_infinity() -> f64 { NEG_INFINITY }

    #[inline]
    fn neg_zero() -> f64 { -0.0 }

    /// Returns `true` if the number is NaN
    #[inline]
    fn is_nan(self) -> bool { self != self }

    /// Returns `true` if the number is infinite
    #[inline]
    fn is_infinite(self) -> bool {
        self == Float::infinity() || self == Float::neg_infinity()
    }

    /// Returns `true` if the number is neither infinite or NaN
    #[inline]
    fn is_finite(self) -> bool {
        !(self.is_nan() || self.is_infinite())
    }

    /// Returns `true` if the number is neither zero, infinite, subnormal or NaN
    #[inline]
    fn is_normal(self) -> bool {
        self.classify() == FPNormal
    }

    /// Returns the floating point category of the number. If only one property
    /// is going to be tested, it is generally faster to use the specific
    /// predicate instead.
    fn classify(self) -> FPCategory {
        static EXP_MASK: u64 = 0x7ff0000000000000;
        static MAN_MASK: u64 = 0x000fffffffffffff;

        let bits: u64 = unsafe { mem::transmute(self) };
        match (bits & MAN_MASK, bits & EXP_MASK) {
            (0, 0)        => FPZero,
            (_, 0)        => FPSubnormal,
            (0, EXP_MASK) => FPInfinite,
            (_, EXP_MASK) => FPNaN,
            _             => FPNormal,
        }
    }

    #[inline]
    fn mantissa_digits(_: Option<f64>) -> uint { MANTISSA_DIGITS }

    #[inline]
    fn digits(_: Option<f64>) -> uint { DIGITS }

    #[inline]
    fn epsilon() -> f64 { EPSILON }

    #[inline]
    fn min_exp(_: Option<f64>) -> int { MIN_EXP }

    #[inline]
    fn max_exp(_: Option<f64>) -> int { MAX_EXP }

    #[inline]
    fn min_10_exp(_: Option<f64>) -> int { MIN_10_EXP }

    #[inline]
    fn max_10_exp(_: Option<f64>) -> int { MAX_10_EXP }

    #[inline]
    fn min_pos_value(_: Option<f64>) -> f64 { MIN_POS_VALUE }

    /// Returns the mantissa, exponent and sign as integers.
    fn integer_decode(self) -> (u64, i16, i8) {
        let bits: u64 = unsafe { mem::transmute(self) };
        let sign: i8 = if bits >> 63 == 0 { 1 } else { -1 };
        let mut exponent: i16 = ((bits >> 52) & 0x7ff) as i16;
        let mantissa = if exponent == 0 {
            (bits & 0xfffffffffffff) << 1
        } else {
            (bits & 0xfffffffffffff) | 0x10000000000000
        };
        // Exponent bias + mantissa shift
        exponent -= 1023 + 52;
        (mantissa, exponent, sign)
    }

    /// Round half-way cases toward `NEG_INFINITY`
    #[inline]
    fn floor(self) -> f64 {
        unsafe { intrinsics::floorf64(self) }
    }

    /// Round half-way cases toward `INFINITY`
    #[inline]
    fn ceil(self) -> f64 {
        unsafe { intrinsics::ceilf64(self) }
    }

    /// Round half-way cases away from `0.0`
    #[inline]
    fn round(self) -> f64 {
        unsafe { intrinsics::roundf64(self) }
    }

    /// The integer part of the number (rounds towards `0.0`)
    #[inline]
    fn trunc(self) -> f64 {
        unsafe { intrinsics::truncf64(self) }
    }

    /// The fractional part of the number, satisfying:
    ///
    /// ```rust
    /// let x = 1.65f64;
    /// assert!(x == x.trunc() + x.fract())
    /// ```
    #[inline]
    fn fract(self) -> f64 { self - self.trunc() }

    /// Fused multiply-add. Computes `(self * a) + b` with only one rounding
    /// error. This produces a more accurate result with better performance than
    /// a separate multiplication operation followed by an add.
    #[inline]
    fn mul_add(self, a: f64, b: f64) -> f64 {
        unsafe { intrinsics::fmaf64(self, a, b) }
    }

    /// The reciprocal (multiplicative inverse) of the number
    #[inline]
    fn recip(self) -> f64 { 1.0 / self }

    #[inline]
    fn powf(self, n: f64) -> f64 {
        unsafe { intrinsics::powf64(self, n) }
    }

    #[inline]
    fn powi(self, n: i32) -> f64 {
        unsafe { intrinsics::powif64(self, n) }
    }

    /// sqrt(2.0)
    #[inline]
    fn sqrt2() -> f64 { consts::SQRT2 }

    /// 1.0 / sqrt(2.0)
    #[inline]
    fn frac_1_sqrt2() -> f64 { consts::FRAC_1_SQRT2 }

    #[inline]
    fn sqrt(self) -> f64 {
        unsafe { intrinsics::sqrtf64(self) }
    }

    #[inline]
    fn rsqrt(self) -> f64 { self.sqrt().recip() }

    /// Archimedes' constant
    #[inline]
    fn tau() -> f64 { consts::TAU }
    /// 2.0 * pi = tau
    #[inline]
    fn two_pi() -> f64 { consts::PI_2 }

    /// tau / 2.0 = pi
    #[inline]
    fn frac_tau_2() -> f64 { consts::FRAC_TAU_2 }
    /// pi = tau / 2.0
    #[inline]
    fn pi() -> f64 { consts::PI }

    /// tau / 3.0 = 2.0 * pi / 3.0
    #[inline]
    fn frac_tau_3() -> f64 { consts::FRAC_TAU_3 }
    /// 2.0 * pi / 3.0 = tau / 3.0
    #[inline]
    fn frac_2pi_3() -> f64 { consts::FRAC_2PI_3 }

    /// tau / 4.0 = pi / 2.0
    #[inline]
    fn frac_tau_4() -> f64 { consts::FRAC_TAU_4 }
    /// pi / 2.0
    #[inline]
    fn frac_pi_2() -> f64 { consts::FRAC_PI_2 }

    /// tau / 6.0 = pi / 3.0
    #[inline]
    fn frac_tau_6() -> f64 { consts::FRAC_TAU_6 }
    /// pi / 3.0 = tau / 6.0
    #[inline]
    fn frac_pi_3() -> f64 { consts::FRAC_PI_3 }

    /// tau / 8.0 = pi / 4.0
    #[inline]
    fn frac_tau_8() -> f64 { consts::FRAC_TAU_8 }
    /// pi / 4.0 = tau / 8.0
    #[inline]
    fn frac_pi_4() -> f64 { consts::FRAC_PI_4 }

    /// tau / 12.0 = pi / 6.0
    #[inline]
    fn frac_tau_12() -> f64 { consts::FRAC_TAU_12 }
    /// pi / 6.0 = tau / 12.0
    #[inline]
    fn frac_pi_6() -> f64 { consts::FRAC_PI_6 }

    /// tau / 16.0 = pi / 8.0
    #[inline]
    fn frac_tau_16() -> f64 { consts::FRAC_TAU_16 }
    /// pi / 8.0 = tau / 16.0
    #[inline]
    fn frac_pi_8() -> f64 { consts::FRAC_PI_8 }

    /// 1.0 / tau = 1.0 / (pi * 2.0)
    #[inline]
    fn frac_1_tau() -> f64 { consts::FRAC_1_TAU }
    /// 1.0 / (pi * 2.0) = 1.0 / tau
    #[inline]
    fn frac_1_2pi() -> f64 { consts::FRAC_1_2PI }

    /// 2.0 / tau = 1.0 / pi
    #[inline]
    fn frac_2_tau() -> f64 { consts::FRAC_2_TAU }
    /// 1.0 / pi = 2.0 / tau
    #[inline]
    fn frac_1_pi() -> f64 { consts::FRAC_1_PI }

    /// 4.0 / tau = 2.0 / pi
    #[inline]
    fn frac_4_tau() -> f64 { consts::FRAC_4_TAU }
    /// 2.0 / pi = 4.0 / tau
    #[inline]
    fn frac_2_pi() -> f64 { consts::FRAC_2_PI }

    /// 2.0 / sqrt(pi)
    #[inline]
    fn frac_2_sqrtpi() -> f64 { consts::FRAC_2_SQRTPI }

    /// Euler's number
    #[inline]
    fn e() -> f64 { consts::E }

    /// log2(e)
    #[inline]
    fn log2_e() -> f64 { consts::LOG2_E }

    /// log10(e)
    #[inline]
    fn log10_e() -> f64 { consts::LOG10_E }

    /// ln(2.0)
    #[inline]
    fn ln_2() -> f64 { consts::LN_2 }

    /// ln(10.0)
    #[inline]
    fn ln_10() -> f64 { consts::LN_10 }

    /// Returns the exponential of the number
    #[inline]
    fn exp(self) -> f64 {
        unsafe { intrinsics::expf64(self) }
    }

    /// Returns 2 raised to the power of the number
    #[inline]
    fn exp2(self) -> f64 {
        unsafe { intrinsics::exp2f64(self) }
    }

    /// Returns the natural logarithm of the number
    #[inline]
    fn ln(self) -> f64 {
        unsafe { intrinsics::logf64(self) }
    }

    /// Returns the logarithm of the number with respect to an arbitrary base
    #[inline]
    fn log(self, base: f64) -> f64 { self.ln() / base.ln() }

    /// Returns the base 2 logarithm of the number
    #[inline]
    fn log2(self) -> f64 {
        unsafe { intrinsics::log2f64(self) }
    }

    /// Returns the base 10 logarithm of the number
    #[inline]
    fn log10(self) -> f64 {
        unsafe { intrinsics::log10f64(self) }
    }


    /// Converts to degrees, assuming the number is in radians
    #[inline]
    fn to_degrees(self) -> f64 { self * (180.0f64 / Float::pi()) }

    /// Converts to radians, assuming the number is in degrees
    #[inline]
    fn to_radians(self) -> f64 {
        let value: f64 = Float::pi();
        self * (value / 180.0)
    }
}

