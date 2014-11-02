// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// ignore-lexer-test FIXME #15679

#![allow(missing_docs)]

use char;
use char::Char;
use clone::Clone;
use from_str::from_str;
use iter::Iterator;
use num;
use num::{Zero, One, cast, Int, Bounded};
use num::{Float, FPNaN, FPInfinite, ToPrimitive};
use option::{None, Option, Some};
use slice::{ImmutableSlice, MutableSlice, CloneableVector};
use str::{Str, StrSlice};
use string::String;
use vec::Vec;

/// A flag that specifies whether to use exponential (scientific) notation.
pub enum ExponentFormat {
    /// Do not use exponential notation.
    ExpNone,
    /// Use exponential notation with the exponent having a base of 10 and the
    /// exponent sign being `e` or `E`. For example, 1000 would be printed
    /// 1e3.
    ExpDec,
    /// Use exponential notation with the exponent having a base of 2 and the
    /// exponent sign being `p` or `P`. For example, 8 would be printed 1p3.
    ExpBin,
}

/// The number of digits used for emitting the fractional part of a number, if
/// any.
pub enum SignificantDigits {
    /// All calculable digits will be printed.
    ///
    /// Note that bignums or fractions may cause a surprisingly large number
    /// of digits to be printed.
    DigAll,

    /// At most the given number of digits will be printed, truncating any
    /// trailing zeroes.
    DigMax(uint),

    /// Precisely the given number of digits will be printed.
    DigExact(uint)
}

/// How to emit the sign of a number.
pub enum SignFormat {
    /// No sign will be printed. The exponent sign will also be emitted.
    SignNone,
    /// `-` will be printed for negative values, but no sign will be emitted
    /// for positive numbers.
    SignNeg,
    /// `+` will be printed for positive values, and `-` will be printed for
    /// negative values.
    SignAll,
}

// Special value strings as [u8] consts.
static INF_BUF:     [u8, ..3] = [b'i', b'n', b'f'];
static POS_INF_BUF: [u8, ..4] = [b'+', b'i', b'n', b'f'];
static NEG_INF_BUF: [u8, ..4] = [b'-', b'i', b'n', b'f'];
static NAN_BUF:     [u8, ..3] = [b'N', b'a', b'N'];

/**
 * Converts an integral number to its string representation as a byte vector.
 * This is meant to be a common base implementation for all integral string
 * conversion functions like `to_string()` or `to_str_radix()`.
 *
 * # Arguments
 * - `num`           - The number to convert. Accepts any number that
 *                     implements the numeric traits.
 * - `radix`         - Base to use. Accepts only the values 2-36.
 * - `sign`          - How to emit the sign. Options are:
 *     - `SignNone`: No sign at all. Basically emits `abs(num)`.
 *     - `SignNeg`:  Only `-` on negative values.
 *     - `SignAll`:  Both `+` on positive, and `-` on negative numbers.
 * - `f`             - a callback which will be invoked for each ascii character
 *                     which composes the string representation of this integer
 *
 * # Return value
 * A tuple containing the byte vector, and a boolean flag indicating
 * whether it represents a special value like `inf`, `-inf`, `NaN` or not.
 * It returns a tuple because there can be ambiguity between a special value
 * and a number representation at higher bases.
 *
 * # Failure
 * - Fails if `radix` < 2 or `radix` > 36.
 */
fn int_to_str_bytes_common<T: Int>(num: T, radix: uint, sign: SignFormat, f: |u8|) {
    assert!(2 <= radix && radix <= 36);

    let _0: T = Zero::zero();

    let neg = num < _0;
    let radix_gen: T = cast(radix).unwrap();

    let mut deccum = num;
    // This is just for integral types, the largest of which is a u64. The
    // smallest base that we can have is 2, so the most number of digits we're
    // ever going to have is 64
    let mut buf = [0u8, ..64];
    let mut cur = 0;

    // Loop at least once to make sure at least a `0` gets emitted.
    loop {
        // Calculate the absolute value of each digit instead of only
        // doing it once for the whole number because a
        // representable negative number doesn't necessary have an
        // representable additive inverse of the same type
        // (See twos complement). But we assume that for the
        // numbers [-35 .. 0] we always have [0 .. 35].
        let current_digit_signed = deccum % radix_gen;
        let current_digit = if current_digit_signed < _0 {
            -current_digit_signed
        } else {
            current_digit_signed
        };
        buf[cur] = match current_digit.to_u8().unwrap() {
            i @ 0...9 => b'0' + i,
            i         => b'a' + (i - 10),
        };
        cur += 1;

        deccum = deccum / radix_gen;
        // No more digits to calculate for the non-fractional part -> break
        if deccum == _0 { break; }
    }

    // Decide what sign to put in front
    match sign {
        SignNeg | SignAll if neg => { f(b'-'); }
        SignAll => { f(b'+'); }
        _ => ()
    }

    // We built the number in reverse order, so un-reverse it here
    while cur > 0 {
        cur -= 1;
        f(buf[cur]);
    }
}

/**
 * Converts a number to its string representation as a byte vector.
 * This is meant to be a common base implementation for all numeric string
 * conversion functions like `to_string()` or `to_str_radix()`.
 *
 * # Arguments
 * - `num`           - The number to convert. Accepts any number that
 *                     implements the numeric traits.
 * - `radix`         - Base to use. Accepts only the values 2-36. If the exponential notation
 *                     is used, then this base is only used for the significand. The exponent
 *                     itself always printed using a base of 10.
 * - `negative_zero` - Whether to treat the special value `-0` as
 *                     `-0` or as `+0`.
 * - `sign`          - How to emit the sign. See `SignFormat`.
 * - `digits`        - The amount of digits to use for emitting the fractional
 *                     part, if any. See `SignificantDigits`.
 * - `exp_format`   - Whether or not to use the exponential (scientific) notation.
 *                    See `ExponentFormat`.
 * - `exp_capital`   - Whether or not to use a capital letter for the exponent sign, if
 *                     exponential notation is desired.
 *
 * # Return value
 * A tuple containing the byte vector, and a boolean flag indicating
 * whether it represents a special value like `inf`, `-inf`, `NaN` or not.
 * It returns a tuple because there can be ambiguity between a special value
 * and a number representation at higher bases.
 *
 * # Failure
 * - Fails if `radix` < 2 or `radix` > 36.
 * - Fails if `radix` > 14 and `exp_format` is `ExpDec` due to conflict
 *   between digit and exponent sign `'e'`.
 * - Fails if `radix` > 25 and `exp_format` is `ExpBin` due to conflict
 *   between digit and exponent sign `'p'`.
 */
pub fn float_to_str_bytes_common<T: Float>(
        num: T, radix: uint, negative_zero: bool,
        sign: SignFormat, digits: SignificantDigits, exp_format: ExponentFormat, exp_upper: bool
        ) -> (Vec<u8>, bool) {
    assert!(2 <= radix && radix <= 36);
    match exp_format {
        ExpDec if radix >= DIGIT_E_RADIX       // decimal exponent 'e'
          => panic!("float_to_str_bytes_common: radix {} incompatible with \
                    use of 'e' as decimal exponent", radix),
        ExpBin if radix >= DIGIT_P_RADIX       // binary exponent 'p'
          => panic!("float_to_str_bytes_common: radix {} incompatible with \
                    use of 'p' as binary exponent", radix),
        _ => ()
    }

    let _0: T = Zero::zero();
    let _1: T = One::one();

    match num.classify() {
        FPNaN => { return (b"NaN".to_vec(), true); }
        FPInfinite if num > _0 => {
            return match sign {
                SignAll => (b"+inf".to_vec(), true),
                _       => (b"inf".to_vec(), true)
            };
        }
        FPInfinite if num < _0 => {
            return match sign {
                SignNone => (b"inf".to_vec(), true),
                _        => (b"-inf".to_vec(), true),
            };
        }
        _ => {}
    }

    let neg = num < _0 || (negative_zero && _1 / num == Float::neg_infinity());
    let mut buf = Vec::new();
    let radix_gen: T   = cast(radix as int).unwrap();

    let (num, exp) = match exp_format {
        ExpNone => (num, 0i32),
        ExpDec | ExpBin => {
            if num == _0 {
                (num, 0i32)
            } else {
                let (exp, exp_base) = match exp_format {
                    ExpDec => (num.abs().log10().floor(), cast::<f64, T>(10.0f64).unwrap()),
                    ExpBin => (num.abs().log2().floor(), cast::<f64, T>(2.0f64).unwrap()),
                    ExpNone => unreachable!()
                };

                (num / exp_base.powf(exp), cast::<T, i32>(exp).unwrap())
            }
        }
    };

    // First emit the non-fractional part, looping at least once to make
    // sure at least a `0` gets emitted.
    let mut deccum = num.trunc();
    loop {
        // Calculate the absolute value of each digit instead of only
        // doing it once for the whole number because a
        // representable negative number doesn't necessary have an
        // representable additive inverse of the same type
        // (See twos complement). But we assume that for the
        // numbers [-35 .. 0] we always have [0 .. 35].
        let current_digit = (deccum % radix_gen).abs();

        // Decrease the deccumulator one digit at a time
        deccum = deccum / radix_gen;
        deccum = deccum.trunc();

        buf.push(char::from_digit(current_digit.to_int().unwrap() as uint, radix)
             .unwrap() as u8);

        // No more digits to calculate for the non-fractional part -> break
        if deccum == _0 { break; }
    }

    // If limited digits, calculate one digit more for rounding.
    let (limit_digits, digit_count, exact) = match digits {
        DigAll          => (false, 0u,      false),
        DigMax(count)   => (true,  count+1, false),
        DigExact(count) => (true,  count+1, true)
    };

    // Decide what sign to put in front
    match sign {
        SignNeg | SignAll if neg => {
            buf.push(b'-');
        }
        SignAll => {
            buf.push(b'+');
        }
        _ => ()
    }

    buf.reverse();

    // Remember start of the fractional digits.
    // Points one beyond end of buf if none get generated,
    // or at the '.' otherwise.
    let start_fractional_digits = buf.len();

    // Now emit the fractional part, if any
    deccum = num.fract();
    if deccum != _0 || (limit_digits && exact && digit_count > 0) {
        buf.push(b'.');
        let mut dig = 0u;

        // calculate new digits while
        // - there is no limit and there are digits left
        // - or there is a limit, it's not reached yet and
        //   - it's exact
        //   - or it's a maximum, and there are still digits left
        while (!limit_digits && deccum != _0)
           || (limit_digits && dig < digit_count && (
                   exact
                || (!exact && deccum != _0)
              )
        ) {
            // Shift first fractional digit into the integer part
            deccum = deccum * radix_gen;

            // Calculate the absolute value of each digit.
            // See note in first loop.
            let current_digit = deccum.trunc().abs();

            buf.push(char::from_digit(
                current_digit.to_int().unwrap() as uint, radix).unwrap() as u8);

            // Decrease the deccumulator one fractional digit at a time
            deccum = deccum.fract();
            dig += 1u;
        }

        // If digits are limited, and that limit has been reached,
        // cut off the one extra digit, and depending on its value
        // round the remaining ones.
        if limit_digits && dig == digit_count {
            let ascii2value = |chr: u8| {
                char::to_digit(chr as char, radix).unwrap()
            };
            let value2ascii = |val: uint| {
                char::from_digit(val, radix).unwrap() as u8
            };

            let extra_digit = ascii2value(buf.pop().unwrap());
            if extra_digit >= radix / 2 { // -> need to round
                let mut i: int = buf.len() as int - 1;
                loop {
                    // If reached left end of number, have to
                    // insert additional digit:
                    if i < 0
                    || buf[i as uint] == b'-'
                    || buf[i as uint] == b'+' {
                        buf.insert((i + 1) as uint, value2ascii(1));
                        break;
                    }

                    // Skip the '.'
                    if buf[i as uint] == b'.' { i -= 1; continue; }

                    // Either increment the digit,
                    // or set to 0 if max and carry the 1.
                    let current_digit = ascii2value(buf[i as uint]);
                    if current_digit < (radix - 1) {
                        buf[i as uint] = value2ascii(current_digit+1);
                        break;
                    } else {
                        buf[i as uint] = value2ascii(0);
                        i -= 1;
                    }
                }
            }
        }
    }

    // if number of digits is not exact, remove all trailing '0's up to
    // and including the '.'
    if !exact {
        let buf_max_i = buf.len() - 1;

        // index to truncate from
        let mut i = buf_max_i;

        // discover trailing zeros of fractional part
        while i > start_fractional_digits && buf[i] == b'0' {
            i -= 1;
        }

        // Only attempt to truncate digits if buf has fractional digits
        if i >= start_fractional_digits {
            // If buf ends with '.', cut that too.
            if buf[i] == b'.' { i -= 1 }

            // only resize buf if we actually remove digits
            if i < buf_max_i {
                buf = buf.slice(0, i + 1).to_vec();
            }
        }
    } // If exact and trailing '.', just cut that
    else {
        let max_i = buf.len() - 1;
        if buf[max_i] == b'.' {
            buf = buf.slice(0, max_i).to_vec();
        }
    }

    match exp_format {
        ExpNone => (),
        _ => {
            buf.push(match exp_format {
                ExpDec if exp_upper => 'E',
                ExpDec if !exp_upper => 'e',
                ExpBin if exp_upper => 'P',
                ExpBin if !exp_upper => 'p',
                _ => unreachable!()
            } as u8);

            int_to_str_bytes_common(exp, 10, sign, |c| buf.push(c));
        }
    }

    (buf, false)
}

/**
 * Converts a number to its string representation. This is a wrapper for
 * `to_str_bytes_common()`, for details see there.
 */
#[inline]
pub fn float_to_str_common<T: Float>(
        num: T, radix: uint, negative_zero: bool,
        sign: SignFormat, digits: SignificantDigits, exp_format: ExponentFormat, exp_capital: bool
        ) -> (String, bool) {
    let (bytes, special) = float_to_str_bytes_common(num, radix,
                               negative_zero, sign, digits, exp_format, exp_capital);
    (String::from_utf8(bytes).unwrap(), special)
}

// Some constants for from_str_bytes_common's input validation,
// they define minimum radix values for which the character is a valid digit.
static DIGIT_P_RADIX: uint = ('p' as uint) - ('a' as uint) + 11u;
static DIGIT_I_RADIX: uint = ('i' as uint) - ('a' as uint) + 11u;
static DIGIT_E_RADIX: uint = ('e' as uint) - ('a' as uint) + 11u;

/**
 * Parses a string as a number. This is meant to
 * be a common base implementation for all numeric string conversion
 * functions like `from_str()` or `from_str_radix()`.
 *
 * # Arguments
 * - `src`        - The string to parse.
 * - `radix`      - Which base to parse the number as. Accepts 2-36.
 * - `special`    - Whether to accept special values like `inf`
 *                  and `NaN`. Can conflict with `radix`, see Failure.
 * - `exponent`   - Which exponent format to accept. Options are:
 *     - `ExpNone`: No Exponent, accepts just plain numbers like `42` or
 *                  `-8.2`.
 *     - `ExpDec`:  Accepts numbers with a decimal exponent like `42e5` or
 *                  `8.2E-2`. The exponent string itself is always base 10.
 *                  Can conflict with `radix`, see Failure.
 *     - `ExpBin`:  Accepts numbers with a binary exponent like `42P-8` or
 *                  `FFp128`. The exponent string itself is always base 10.
 *                  Can conflict with `radix`, see Failure.
 *
 * # Return value
 * Returns `Some(n)` if `buf` parses to a number n without overflowing, and
 * `None` otherwise, depending on the constraints set by the remaining
 * arguments.
 *
 * # Failure
 * - Fails if `radix` < 2 or `radix` > 36.
 * - Fails if `radix` > 14 and `exponent` is `ExpDec` due to conflict
 *   between digit and exponent sign `'e'`.
 * - Fails if `radix` > 25 and `exponent` is `ExpBin` due to conflict
 *   between digit and exponent sign `'p'`.
 * - Fails if `radix` > 18 and `special == true` due to conflict
 *   between digit and lowest first character in `inf` and `NaN`, the `'i'`.
 */
pub fn from_str_float<T: Float>(
        src: &str, radix: uint, special: bool, exponent: ExponentFormat,
        ) -> Option<T> {
    match exponent {
        ExpDec if radix >= DIGIT_E_RADIX       // decimal exponent 'e'
          => panic!("from_str_bytes_common: radix {} incompatible with \
                    use of 'e' as decimal exponent", radix),
        ExpBin if radix >= DIGIT_P_RADIX       // binary exponent 'p'
          => panic!("from_str_bytes_common: radix {} incompatible with \
                    use of 'p' as binary exponent", radix),
        _ if special && radix >= DIGIT_I_RADIX // first digit of 'inf'
          => panic!("from_str_bytes_common: radix {} incompatible with \
                    special values 'inf' and 'NaN'", radix),
        _ if (radix as int) < 2
          => panic!("from_str_bytes_common: radix {} to low, \
                    must lie in the range [2, 36]", radix),
        _ if (radix as int) > 36
          => panic!("from_str_bytes_common: radix {} to high, \
                    must lie in the range [2, 36]", radix),
        _ => ()
    }

    let _0: T = Zero::zero();
    let _1: T = One::one();
    let radix_gen: T = cast(radix as int).unwrap();
    let buf = src.as_bytes();

    let len = buf.len();

    if len == 0 {
        return None;
    }

    if special {
        if buf == INF_BUF || buf == POS_INF_BUF {
            return Some(Float::infinity());
        } else if buf == NEG_INF_BUF {
            return Some(Float::neg_infinity());
        } else if buf == NAN_BUF {
            return Some(Float::nan());
        }
    }

    let (start, accum_positive) = match buf[0] as char {
      '-' => (1u, false),
      '+' => (1u, true),
       _  => (0u, true)
    };

    // Initialize accumulator with signed zero for floating point parsing to
    // work
    let mut accum      = if accum_positive { _0.clone() } else { -_1 * _0};
    let mut last_accum = accum.clone(); // Necessary to detect overflow
    let mut i          = start;
    let mut exp_found  = false;

    // Parse integer part of number
    while i < len {
        let c = buf[i] as char;

        match char::to_digit(c, radix) {
            Some(digit) => {
                // shift accum one digit left
                accum = accum * radix_gen.clone();

                // add/subtract current digit depending on sign
                if accum_positive {
                    accum = accum + cast(digit as int).unwrap();
                } else {
                    accum = accum - cast(digit as int).unwrap();
                }

                // Detect overflow by comparing to last value, except
                // if we've not seen any non-zero digits.
                if last_accum != _0 {
                    if accum_positive && accum <= last_accum { return Some(Float::infinity()); }
                    if !accum_positive && accum >= last_accum { return Some(Float::neg_infinity()); }

                    // Detect overflow by reversing the shift-and-add process
                    if accum_positive &&
                        (last_accum != ((accum - cast(digit as int).unwrap())/radix_gen.clone())) {
                        return Some(Float::infinity());
                    }
                    if !accum_positive &&
                        (last_accum != ((accum + cast(digit as int).unwrap())/radix_gen.clone())) {
                        return Some(Float::neg_infinity());
                    }
                }
                last_accum = accum.clone();
            }
            None => match c {
                'e' | 'E' | 'p' | 'P' => {
                    exp_found = true;
                    break;                       // start of exponent
                }
                '.' => {
                    i += 1u;                     // skip the '.'
                    break;                       // start of fractional part
                }
                _ => return None                 // invalid number
            }
        }

        i += 1u;
    }

    // Parse fractional part of number
    // Skip if already reached start of exponent
    if !exp_found {
        let mut power = _1.clone();

        while i < len {
            let c = buf[i] as char;

            match char::to_digit(c, radix) {
                Some(digit) => {
                    // Decrease power one order of magnitude
                    power = power / radix_gen;

                    let digit_t: T = cast(digit).unwrap();

                    // add/subtract current digit depending on sign
                    if accum_positive {
                        accum = accum + digit_t * power;
                    } else {
                        accum = accum - digit_t * power;
                    }

                    // Detect overflow by comparing to last value
                    if accum_positive && accum < last_accum { return Some(Float::infinity()); }
                    if !accum_positive && accum > last_accum { return Some(Float::neg_infinity()); }
                    last_accum = accum.clone();
                }
                None => match c {
                    'e' | 'E' | 'p' | 'P' => {
                        exp_found = true;
                        break;                   // start of exponent
                    }
                    _ => return None             // invalid number
                }
            }

            i += 1u;
        }
    }

    // Special case: buf not empty, but does not contain any digit in front
    // of the exponent sign -> number is empty string
    if i == start {
        return None;
    }

    let mut multiplier = _1.clone();

    if exp_found {
        let c = buf[i] as char;
        let base: T = match (c, exponent) {
            // c is never _ so don't need to handle specially
            ('e', ExpDec) | ('E', ExpDec) => cast(10u).unwrap(),
            ('p', ExpBin) | ('P', ExpBin) => cast(2u).unwrap(),
            _ => return None // char doesn't fit given exponent format
        };

        // parse remaining bytes as decimal integer,
        // skipping the exponent char
        let exp = from_str::<int>(String::from_utf8_lossy(buf[i+1..len]).as_slice());

        match exp {
            Some(exp_pow) => {
                multiplier = if exp_pow < 0 {
                    _1 / num::pow(base, (-exp_pow.to_int().unwrap()) as uint)
                } else {
                    num::pow(base, exp_pow.to_int().unwrap() as uint)
                }
            }
            None => return None // invalid exponent -> invalid number
        }
    }

    Some(accum * multiplier)
}

pub fn from_str_radix_int<T: Int>(src: &str, radix: uint) -> Option<T> {
    fn cast<T: Int>(x: uint) -> T {
        num::cast(x).unwrap()
    }

    let _0: T = num::zero();
    let _1: T = num::one();
    let is_signed = _0 > Bounded::min_value();

    let (is_negative, src) =  match src.slice_shift_char() {
        (Some('-'), src) if is_signed => (true, src),
        (Some(_), _) => (false, src),
        (None, _) => return None,
    };

    let mut xs = src.chars().map(|c| {
        c.to_digit(radix).map(cast)
    });
    let radix = cast(radix);
    let mut result = _0;

    if is_negative {
        for x in xs {
            let x = match x {
                Some(x) => x,
                None => return None,
            };
            result = match result.checked_mul(&radix) {
                Some(result) => result,
                None => return None,
            };
            result = match result.checked_sub(&x) {
                Some(result) => result,
                None => return None,
            };
        }
    } else {
        for x in xs {
            let x = match x {
                Some(x) => x,
                None => return None,
            };
            result = match result.checked_mul(&radix) {
                Some(result) => result,
                None => return None,
            };
            result = match result.checked_add(&x) {
                Some(result) => result,
                None => return None,
            };
        }
    }

    Some(result)
}

#[cfg(test)]
mod test {
    use super::*;
    use option::*;
    use num::Float;

    #[test]
    fn from_str_issue7588() {
        let u : Option<u8> = from_str_radix_int("1000", 10);
        assert_eq!(u, None);
        let s : Option<i16> = from_str_radix_int("80000", 10);
        assert_eq!(s, None);
        let f : Option<f32> = from_str_float(
            "10000000000000000000000000000000000000000", 10, false, ExpNone);
        assert_eq!(f, Some(Float::infinity()))
        let fe : Option<f32> = from_str_float("1e40", 10, false, ExpDec);
        assert_eq!(fe, Some(Float::infinity()))
    }
}

#[cfg(test)]
mod bench {
    extern crate test;

    mod uint {
        use super::test::Bencher;
        use rand::{weak_rng, Rng};
        use std::fmt;

        #[inline]
        fn to_string(x: uint, base: u8) {
            format!("{}", fmt::radix(x, base));
        }

        #[bench]
        fn to_str_bin(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { to_string(rng.gen::<uint>(), 2); })
        }

        #[bench]
        fn to_str_oct(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { to_string(rng.gen::<uint>(), 8); })
        }

        #[bench]
        fn to_str_dec(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { to_string(rng.gen::<uint>(), 10); })
        }

        #[bench]
        fn to_str_hex(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { to_string(rng.gen::<uint>(), 16); })
        }

        #[bench]
        fn to_str_base_36(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { to_string(rng.gen::<uint>(), 36); })
        }
    }

    mod int {
        use super::test::Bencher;
        use rand::{weak_rng, Rng};
        use std::fmt;

        #[inline]
        fn to_string(x: int, base: u8) {
            format!("{}", fmt::radix(x, base));
        }

        #[bench]
        fn to_str_bin(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { to_string(rng.gen::<int>(), 2); })
        }

        #[bench]
        fn to_str_oct(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { to_string(rng.gen::<int>(), 8); })
        }

        #[bench]
        fn to_str_dec(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { to_string(rng.gen::<int>(), 10); })
        }

        #[bench]
        fn to_str_hex(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { to_string(rng.gen::<int>(), 16); })
        }

        #[bench]
        fn to_str_base_36(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { to_string(rng.gen::<int>(), 36); })
        }
    }

    mod f64 {
        use super::test::Bencher;
        use rand::{weak_rng, Rng};
        use f64;

        #[bench]
        fn float_to_string(b: &mut Bencher) {
            let mut rng = weak_rng();
            b.iter(|| { f64::to_string(rng.gen()); })
        }
    }
}
