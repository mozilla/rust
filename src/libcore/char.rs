// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Character manipulation.
//!
//! For more details, see ::unicode::char (a.k.a. std::char)

#![allow(non_snake_case)]
#![doc(primitive = "char")]

use clone::Clone;
use mem::transmute;
use option::{None, Option, Some};
use iter::{range_step, Iterator, RangeStep};
use collections::Collection;

// UTF-8 ranges and tags for encoding characters
static TAG_CONT: u8    = 0b1000_0000u8;
static TAG_TWO_B: u8   = 0b1100_0000u8;
static TAG_THREE_B: u8 = 0b1110_0000u8;
static TAG_FOUR_B: u8  = 0b1111_0000u8;
static MAX_ONE_B: u32   =     0x80u32;
static MAX_TWO_B: u32   =    0x800u32;
static MAX_THREE_B: u32 =  0x10000u32;

/*
    Lu  Uppercase_Letter        an uppercase letter
    Ll  Lowercase_Letter        a lowercase letter
    Lt  Titlecase_Letter        a digraphic character, with first part uppercase
    Lm  Modifier_Letter         a modifier letter
    Lo  Other_Letter            other letters, including syllables and ideographs
    Mn  Nonspacing_Mark         a nonspacing combining mark (zero advance width)
    Mc  Spacing_Mark            a spacing combining mark (positive advance width)
    Me  Enclosing_Mark          an enclosing combining mark
    Nd  Decimal_Number          a decimal digit
    Nl  Letter_Number           a letterlike numeric character
    No  Other_Number            a numeric character of other type
    Pc  Connector_Punctuation   a connecting punctuation mark, like a tie
    Pd  Dash_Punctuation        a dash or hyphen punctuation mark
    Ps  Open_Punctuation        an opening punctuation mark (of a pair)
    Pe  Close_Punctuation       a closing punctuation mark (of a pair)
    Pi  Initial_Punctuation     an initial quotation mark
    Pf  Final_Punctuation       a final quotation mark
    Po  Other_Punctuation       a punctuation mark of other type
    Sm  Math_Symbol             a symbol of primarily mathematical use
    Sc  Currency_Symbol         a currency sign
    Sk  Modifier_Symbol         a non-letterlike modifier symbol
    So  Other_Symbol            a symbol of other type
    Zs  Space_Separator         a space character (of various non-zero widths)
    Zl  Line_Separator          U+2028 LINE SEPARATOR only
    Zp  Paragraph_Separator     U+2029 PARAGRAPH SEPARATOR only
    Cc  Control                 a C0 or C1 control code
    Cf  Format                  a format control character
    Cs  Surrogate               a surrogate code point
    Co  Private_Use             a private-use character
    Cn  Unassigned              a reserved unassigned code point or a noncharacter
*/

/// The highest valid code point
#[stable]
pub const MAX: char = '\U0010ffff';

/// Converts from `u32` to a `char`
#[inline]
#[unstable = "pending decisions about costructors for primitives"]
pub fn from_u32(i: u32) -> Option<char> {
    // catch out-of-bounds and surrogates
    if (i > MAX as u32) || (i >= 0xD800 && i <= 0xDFFF) {
        None
    } else {
        Some(unsafe { transmute(i) })
    }
}

///
/// Checks if a `char` parses as a numeric digit in the given radix
///
/// Compared to `is_digit()`, this function only recognizes the
/// characters `0-9`, `a-z` and `A-Z`.
///
/// # Return value
///
/// Returns `true` if `c` is a valid digit under `radix`, and `false`
/// otherwise.
///
/// # Failure
///
/// Fails if given a `radix` > 36.
///
/// # Note
///
/// This just wraps `to_digit()`.
///
#[inline]
#[deprecated = "use the Char::is_digit method"]
pub fn is_digit_radix(c: char, radix: uint) -> bool {
    c.is_digit(radix)
}

///
/// Converts a `char` to the corresponding digit
///
/// # Return value
///
/// If `c` is between '0' and '9', the corresponding value
/// between 0 and 9. If `c` is 'a' or 'A', 10. If `c` is
/// 'b' or 'B', 11, etc. Returns none if the `char` does not
/// refer to a digit in the given radix.
///
/// # Failure
///
/// Fails if given a `radix` outside the range `[0..36]`.
///
#[inline]
#[deprecated = "use the Char::to_digit method"]
pub fn to_digit(c: char, radix: uint) -> Option<uint> {
    c.to_digit(radix)
}

///
/// Converts a number to the character representing it
///
/// # Return value
///
/// Returns `Some(char)` if `num` represents one digit under `radix`,
/// using one character of `0-9` or `a-z`, or `None` if it doesn't.
///
/// # Failure
///
/// Fails if given an `radix` > 36.
///
#[inline]
#[unstable = "pending decisions about costructors for primitives"]
pub fn from_digit(num: uint, radix: uint) -> Option<char> {
    if radix > 36 {
        fail!("from_digit: radix is too high (maximum 36)");
    }
    if num < radix {
        unsafe {
            if num < 10 {
                Some(transmute(('0' as uint + num) as u32))
            } else {
                Some(transmute(('a' as uint + num - 10u) as u32))
            }
        }
    } else {
        None
    }
}

///
/// Returns the hexadecimal Unicode escape of a `char`
///
/// The rules are as follows:
///
/// - chars in [0,0xff] get 2-digit escapes: `\\xNN`
/// - chars in [0x100,0xffff] get 4-digit escapes: `\\uNNNN`
/// - chars above 0x10000 get 8-digit escapes: `\\UNNNNNNNN`
///
#[deprecated = "use the Char::escape_unicode method"]
pub fn escape_unicode(c: char, f: |char|) {
    for char in c.escape_unicode() {
        f(char);
    }
}

///
/// Returns a 'default' ASCII and C++11-like literal escape of a `char`
///
/// The default is chosen with a bias toward producing literals that are
/// legal in a variety of languages, including C++11 and similar C-family
/// languages. The exact rules are:
///
/// - Tab, CR and LF are escaped as '\t', '\r' and '\n' respectively.
/// - Single-quote, double-quote and backslash chars are backslash-escaped.
/// - Any other chars in the range [0x20,0x7e] are not escaped.
/// - Any other chars are given hex Unicode escapes; see `escape_unicode`.
///
#[deprecated = "use the Char::escape_default method"]
pub fn escape_default(c: char, f: |char|) {
    for c in c.escape_default() {
        f(c);
    }
}

/// Returns the amount of bytes this `char` would need if encoded in UTF-8
#[inline]
#[deprecated = "use the Char::len_utf8 method"]
pub fn len_utf8_bytes(c: char) -> uint {
    c.len_utf8()
}

/// Basic `char` manipulations.
#[experimental = "trait organization may change"]
pub trait Char {
    /// Checks if a `char` parses as a numeric digit in the given radix.
    ///
    /// Compared to `is_digit()`, this function only recognizes the characters
    /// `0-9`, `a-z` and `A-Z`.
    ///
    /// # Return value
    ///
    /// Returns `true` if `c` is a valid digit under `radix`, and `false`
    /// otherwise.
    ///
    /// # Failure
    ///
    /// Fails if given a radix > 36.
    #[deprecated = "use is_digit"]
    fn is_digit_radix(self, radix: uint) -> bool;

    /// Checks if a `char` parses as a numeric digit in the given radix.
    ///
    /// Compared to `is_digit()`, this function only recognizes the characters
    /// `0-9`, `a-z` and `A-Z`.
    ///
    /// # Return value
    ///
    /// Returns `true` if `c` is a valid digit under `radix`, and `false`
    /// otherwise.
    ///
    /// # Failure
    ///
    /// Fails if given a radix > 36.
    #[unstable = "pending error conventions"]
    fn is_digit(self, radix: uint) -> bool;

    /// Converts a character to the corresponding digit.
    ///
    /// # Return value
    ///
    /// If `c` is between '0' and '9', the corresponding value between 0 and
    /// 9. If `c` is 'a' or 'A', 10. If `c` is 'b' or 'B', 11, etc. Returns
    /// none if the character does not refer to a digit in the given radix.
    ///
    /// # Failure
    ///
    /// Fails if given a radix outside the range [0..36].
    #[unstable = "pending error conventions, trait organization"]
    fn to_digit(self, radix: uint) -> Option<uint>;

    /// Converts a number to the character representing it.
    ///
    /// # Return value
    ///
    /// Returns `Some(char)` if `num` represents one digit under `radix`,
    /// using one character of `0-9` or `a-z`, or `None` if it doesn't.
    ///
    /// # Failure
    ///
    /// Fails if given a radix > 36.
    #[deprecated = "use the char::from_digit free function"]
    fn from_digit(num: uint, radix: uint) -> Option<Self>;

    /// Converts from `u32` to a `char`
    #[deprecated = "use the char::from_u32 free function"]
    fn from_u32(i: u32) -> Option<char>;

    /// Returns the hexadecimal Unicode escape of a character.
    ///
    /// The rules are as follows:
    ///
    /// * Characters in [0,0xff] get 2-digit escapes: `\\xNN`
    /// * Characters in [0x100,0xffff] get 4-digit escapes: `\\uNNNN`.
    /// * Characters above 0x10000 get 8-digit escapes: `\\UNNNNNNNN`.
    #[unstable = "pending error conventions, trait organization"]
    fn escape_unicode(self) -> UnicodeEscapedChars;

    /// Returns a 'default' ASCII and C++11-like literal escape of a
    /// character.
    ///
    /// The default is chosen with a bias toward producing literals that are
    /// legal in a variety of languages, including C++11 and similar C-family
    /// languages. The exact rules are:
    ///
    /// * Tab, CR and LF are escaped as '\t', '\r' and '\n' respectively.
    /// * Single-quote, double-quote and backslash chars are backslash-
    ///   escaped.
    /// * Any other chars in the range [0x20,0x7e] are not escaped.
    /// * Any other chars are given hex Unicode escapes; see `escape_unicode`.
    #[unstable = "pending error conventions, trait organization"]
    fn escape_default(self) -> DefaultEscapedChars;

    /// Returns the amount of bytes this character would need if encoded in
    /// UTF-8.
    #[deprecated = "use len_utf8"]
    fn len_utf8_bytes(self) -> uint;

    /// Returns the amount of bytes this character would need if encoded in
    /// UTF-8.
    #[unstable = "pending trait organization"]
    fn len_utf8(self) -> uint;

    /// Returns the amount of bytes this character would need if encoded in
    /// UTF-16.
    #[unstable = "pending trait organization"]
    fn len_utf16(self) -> uint;

    /// Encodes this character as UTF-8 into the provided byte buffer,
    /// and then returns the number of bytes written.
    ///
    /// If the buffer is not large enough, nothing will be written into it
    /// and a `None` will be returned.
    #[unstable = "pending trait organization"]
    fn encode_utf8(self) -> Utf8CodeUnits;

    /// Encodes this character as UTF-16 into the provided `u16` buffer,
    /// and then returns the number of `u16`s written.
    ///
    /// If the buffer is not large enough, nothing will be written into it
    /// and a `None` will be returned.
    #[unstable = "pending trait organization"]
    fn encode_utf16(self) -> Utf16CodeUnits;
}

#[experimental = "trait is experimental"]
impl Char for char {
    #[deprecated = "use is_digit"]
    fn is_digit_radix(self, radix: uint) -> bool { self.is_digit(radix) }

    #[unstable = "pending trait organization"]
    fn is_digit(self, radix: uint) -> bool {
        match self.to_digit(radix) {
            Some(_) => true,
            None    => false,
        }
    }

    #[unstable = "pending trait organization"]
    fn to_digit(self, radix: uint) -> Option<uint> {
        if radix > 36 {
            fail!("to_digit: radix is too high (maximum 36)");
        }
        let val = match self {
          '0' ... '9' => self as uint - ('0' as uint),
          'a' ... 'z' => self as uint + 10u - ('a' as uint),
          'A' ... 'Z' => self as uint + 10u - ('A' as uint),
          _ => return None,
        };
        if val < radix { Some(val) }
        else { None }
    }

    #[deprecated = "use the char::from_digit free function"]
    fn from_digit(num: uint, radix: uint) -> Option<char> { from_digit(num, radix) }

    #[inline]
    #[deprecated = "use the char::from_u32 free function"]
    fn from_u32(i: u32) -> Option<char> { from_u32(i) }

    #[unstable = "pending error conventions, trait organization"]
    fn escape_unicode(self) -> UnicodeEscapedChars {
        UnicodeEscapedChars { c: self, state: EscapeBackslash }
    }

    #[unstable = "pending error conventions, trait organization"]
    fn escape_default(self) -> DefaultEscapedChars {
        let init_state = match self {
            '\t' => DefaultEscapeBackslash('t'),
            '\r' => DefaultEscapeBackslash('r'),
            '\n' => DefaultEscapeBackslash('n'),
            '\\' => DefaultEscapeBackslash('\\'),
            '\'' => DefaultEscapeBackslash('\''),
            '"'  => DefaultEscapeBackslash('"'),
            '\x20' ... '\x7e' => DefaultEscapeChar(self),
            _ => DefaultEscapeUnicode(self.escape_unicode())
        };
        DefaultEscapedChars { state: init_state }
    }

    #[inline]
    #[deprecated = "use len_utf8"]
    fn len_utf8_bytes(self) -> uint { self.len_utf8() }

    #[inline]
    #[unstable = "pending trait organization"]
    fn len_utf8(self) -> uint {
        let code = self as u32;
        match () {
            _ if code < MAX_ONE_B   => 1u,
            _ if code < MAX_TWO_B   => 2u,
            _ if code < MAX_THREE_B => 3u,
            _  => 4u,
        }
    }

    #[inline]
    #[unstable = "pending trait organization"]
    fn len_utf16(self) -> uint {
        let ch = self as u32;
        if (ch & 0xFFFF_u32) == ch { 1 } else { 2 }
    }

    #[inline]
    #[unstable = "pending error conventions, trait organization"]
    fn encode_utf8(self) -> Utf8CodeUnits {
        let code = self as u32;
        let (len, buf) = if code < MAX_ONE_B {
            (1, [code as u8, 0, 0, 0])
        } else if code < MAX_TWO_B {
            (2, [(code >> 6u & 0x1F_u32) as u8 | TAG_TWO_B,
                 (code & 0x3F_u32) as u8 | TAG_CONT,
                 0, 0])
        } else if code < MAX_THREE_B {
            (3, [(code >> 12u & 0x0F_u32) as u8 | TAG_THREE_B,
                 (code >>  6u & 0x3F_u32) as u8 | TAG_CONT,
                 (code & 0x3F_u32) as u8 | TAG_CONT,
                 0])
        } else {
            (4, [(code >> 18u & 0x07_u32) as u8 | TAG_FOUR_B,
                 (code >> 12u & 0x3F_u32) as u8 | TAG_CONT,
                 (code >>  6u & 0x3F_u32) as u8 | TAG_CONT,
                 (code & 0x3F_u32) as u8 | TAG_CONT])
        };

        Utf8CodeUnits { pos: 0, len: len, buf: buf }
    }

    #[inline]
    #[unstable = "pending error conventions, trait organization"]
    fn encode_utf16(self) -> Utf16CodeUnits {
        // Marked #[inline] to allow llvm optimizing it away
        let mut ch = self as u32;
        let (len, buf) = if (ch & 0xFFFF_u32) == ch {
            // The BMP falls through (assuming non-surrogate, as it should)
            (1, [ch as u16, 0])
        } else {
            // Supplementary planes break into surrogates.
            ch -= 0x1_0000_u32;
            (2, [0xD800_u16 | ((ch >> 10) as u16),
                 0xDC00_u16 | ((ch as u16) & 0x3FF_u16)])
        };

        Utf16CodeUnits { pos: 0, len: len, buf: buf }
    }
}

/// An iterator over the bytes of a char encoded as UTF-8
#[unstable = "pending error conventions, trait organization"]
pub struct Utf8CodeUnits {
    pos: uint,
    len: uint,
    buf: [u8, ..4]
}

#[unstable = "struct is unstable"]
impl Iterator<u8> for Utf8CodeUnits {
    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.pos != self.len {
            let next = self.buf[self.pos];
            self.pos += 1;
            Some(next)
        } else {
            None
        }
    }
}

#[unstable = "struct is unstable"]
impl Clone for Utf8CodeUnits {
    fn clone(&self) -> Utf8CodeUnits {
        Utf8CodeUnits { pos: self.pos, len: self.len, buf: self.buf }
    }
}

/// An iterator over the bytes of a char encoded as UTF-8
#[unstable = "pending error conventions, trait organization"]
pub struct Utf16CodeUnits {
    pos: uint,
    len: uint,
    buf: [u16, ..2]
}

#[unstable = "struct is unstable"]
impl Iterator<u16> for Utf16CodeUnits {
    #[inline]
    fn next(&mut self) -> Option<u16> {
        if self.pos != self.len {
            let next = self.buf[self.pos];
            self.pos += 1;
            Some(next)
        } else {
            None
        }
    }
}

#[unstable = "struct is unstable"]
impl Clone for Utf16CodeUnits {
    fn clone(&self) -> Utf16CodeUnits {
        Utf16CodeUnits { pos: self.pos, len: self.len, buf: self.buf }
    }
}

/// An iterator over the characters that represent a `char`, as escaped by
/// Rust's unicode escaping rules.
pub struct UnicodeEscapedChars {
    c: char,
    state: UnicodeEscapedCharsState
}

enum UnicodeEscapedCharsState {
    EscapeBackslash,
    EscapeType,
    EscapeValue(RangeStep<i32>),
}

impl Iterator<char> for UnicodeEscapedChars {
    fn next(&mut self) -> Option<char> {
        match self.state {
            EscapeBackslash => {
                self.state = EscapeType;
                Some('\\')
            }
            EscapeType => {
                let (typechar, pad) = if self.c <= '\xff' { ('x', 2) }
                                      else if self.c <= '\uffff' { ('u', 4) }
                                      else { ('U', 8) };
                self.state = EscapeValue(range_step(4 * (pad - 1), -1, -4i32));
                Some(typechar)
            }
            EscapeValue(ref mut range_step) => match range_step.next() {
                Some(offset) => {
                    let offset = offset as uint;
                    let v = match ((self.c as i32) >> offset) & 0xf {
                        i @ 0 ... 9 => '0' as i32 + i,
                        i => 'a' as i32 + (i - 10)
                    };
                    Some(unsafe { transmute(v) })
                }
                None => None
            }
        }
    }    
}

/// An iterator over the characters that represent a `char`, escaped
/// for maximum portability.
pub struct DefaultEscapedChars {
    state: DefaultEscapedCharsState
}

enum DefaultEscapedCharsState {
    DefaultEscapeBackslash(char),
    DefaultEscapeChar(char),
    DefaultEscapeDone,
    DefaultEscapeUnicode(UnicodeEscapedChars),
}

impl Iterator<char> for DefaultEscapedChars {
    fn next(&mut self) -> Option<char> {
        match self.state {
            DefaultEscapeBackslash(c) => {
                self.state = DefaultEscapeChar(c);
                Some('\\')
            }
            DefaultEscapeChar(c) => {
                self.state = DefaultEscapeDone;
                Some(c)
            }
            DefaultEscapeDone => None,
            DefaultEscapeUnicode(ref mut iter) => iter.next()
        }
    }
}

