//! Operations on ASCII strings and characters.
//!
//! Most string operations in Rust act on UTF-8 strings. However, at times it
//! makes more sense to only consider the ASCII character set for a specific
//! operation.
//!
//! The [`escape_default`] function provides an iterator over the bytes of an
//! escaped version of the character given.
//!
//! [`escape_default`]: fn.escape_default.html

#![stable(feature = "core_ascii", since = "1.26.0")]

use crate::fmt;
use crate::iter::FusedIterator;
use crate::ops::Range;
use crate::str::from_utf8_unchecked;

/// An iterator over the escaped version of a byte.
///
/// This `struct` is created by the [`escape_default`] function. See its
/// documentation for more.
///
/// [`escape_default`]: fn.escape_default.html
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Clone)]
pub struct EscapeDefault {
    range: Range<usize>,
    data: [u8; 4],
}

/// Returns an iterator that produces an escaped version of a `u8`.
///
/// The default is chosen with a bias toward producing literals that are
/// legal in a variety of languages, including C++11 and similar C-family
/// languages. The exact rules are:
///
/// * Tab is escaped as `\t`.
/// * Carriage return is escaped as `\r`.
/// * Line feed is escaped as `\n`.
/// * Single quote is escaped as `\'`.
/// * Double quote is escaped as `\"`.
/// * Backslash is escaped as `\\`.
/// * Any character in the 'printable ASCII' range `0x20` .. `0x7e`
///   inclusive is not escaped.
/// * Any other chars are given hex escapes of the form '\xNN'.
/// * Unicode escapes are never generated by this function.
///
/// # Examples
///
/// ```
/// use std::ascii;
///
/// let escaped = ascii::escape_default(b'0').next().unwrap();
/// assert_eq!(b'0', escaped);
///
/// let mut escaped = ascii::escape_default(b'\t');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b't', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'\r');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'r', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'\n');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'n', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'\'');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'\'', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'"');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'"', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'\\');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'\\', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'\x9d');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'x', escaped.next().unwrap());
/// assert_eq!(b'9', escaped.next().unwrap());
/// assert_eq!(b'd', escaped.next().unwrap());
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub fn escape_default(c: u8) -> EscapeDefault {
    let (data, len) = match c {
        b'\t' => ([b'\\', b't', 0, 0], 2),
        b'\r' => ([b'\\', b'r', 0, 0], 2),
        b'\n' => ([b'\\', b'n', 0, 0], 2),
        b'\\' => ([b'\\', b'\\', 0, 0], 2),
        b'\'' => ([b'\\', b'\'', 0, 0], 2),
        b'"' => ([b'\\', b'"', 0, 0], 2),
        b'\x20'..=b'\x7e' => ([c, 0, 0, 0], 1),
        _ => ([b'\\', b'x', hexify(c >> 4), hexify(c & 0xf)], 4),
    };

    return EscapeDefault { range: 0..len, data };

    fn hexify(b: u8) -> u8 {
        match b {
            0..=9 => b'0' + b,
            _ => b'a' + b - 10,
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Iterator for EscapeDefault {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        self.range.next().map(|i| self.data[i])
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
    fn last(mut self) -> Option<u8> {
        self.next_back()
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl DoubleEndedIterator for EscapeDefault {
    fn next_back(&mut self) -> Option<u8> {
        self.range.next_back().map(|i| self.data[i])
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl ExactSizeIterator for EscapeDefault {}
#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for EscapeDefault {}

#[stable(feature = "ascii_escape_display", since = "1.39.0")]
impl fmt::Display for EscapeDefault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: ok because `escape_default` created only valid utf-8 data
        f.write_str(unsafe { from_utf8_unchecked(&self.data[self.range.clone()]) })
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for EscapeDefault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("EscapeDefault { .. }")
    }
}
