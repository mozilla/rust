// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Terminfo database interface.

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io;
use std::io::BufReader;
use std::path::Path;

use Attr;
use color;
use Terminal;
use Result;
use self::searcher::get_dbpath_for_term;
use self::parser::compiled::{parse, msys_terminfo};
use self::parm::{expand, Variables, Param};
use self::Error::*;


/// A parsed terminfo database entry.
#[derive(Debug)]
pub struct TermInfo {
    /// Names for the terminal
    pub names: Vec<String>,
    /// Map of capability name to boolean value
    pub bools: HashMap<String, bool>,
    /// Map of capability name to numeric value
    pub numbers: HashMap<String, u16>,
    /// Map of capability name to raw (unexpanded) string
    pub strings: HashMap<String, Vec<u8>>,
}

impl TermInfo {
    /// Create a TermInfo based on current environment.
    pub fn from_env() -> Result<TermInfo> {
        let term = match env::var("TERM") {
            Ok(name) => TermInfo::from_name(&name),
            Err(..) => return Err(::Error::TermUnset),
        };

        if term.is_err() && env::var("MSYSCON").ok().map_or(false, |s| "mintty.exe" == s) {
            // msys terminal
            Ok(msys_terminfo())
        } else {
            term
        }
    }

    /// Create a TermInfo for the named terminal.
    pub fn from_name(name: &str) -> Result<TermInfo> {
        get_dbpath_for_term(name)
            .ok_or_else(|| ::Error::TerminfoEntryNotFound)
            .and_then(|p| TermInfo::from_path(&p))
    }

    /// Parse the given TermInfo.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<TermInfo> {
        Self::_from_path(path.as_ref())
    }
    // Keep the metadata small
    // (That is, this uses a &Path so that this function need not be instantiated
    // for every type
    // which implements AsRef<Path>. One day, if/when rustc is a bit smarter, it
    // might do this for
    // us. Alas. )
    fn _from_path(path: &Path) -> Result<TermInfo> {
        let file = try!(File::open(path).map_err(|e| ::Error::Io(e)));
        let mut reader = BufReader::new(file);
        parse(&mut reader, false)
    }
}

#[derive(Debug, Eq, PartialEq)]
/// An error from parsing a terminfo entry
pub enum Error {
    /// The "magic" number at the start of the file was wrong.
    ///
    /// It should be `0x11A`
    BadMagic(u16),
    /// The names in the file were not valid UTF-8.
    ///
    /// In theory these should only be ASCII, but to work with the Rust `str` type, we treat them
    /// as UTF-8. This is valid, except when a terminfo file decides to be invalid. This hasn't
    /// been encountered in the wild.
    NotUtf8(::std::str::Utf8Error),
    /// The names section of the file was empty
    ShortNames,
    /// More boolean parameters are present in the file than this crate knows how to interpret.
    TooManyBools,
    /// More number parameters are present in the file than this crate knows how to interpret.
    TooManyNumbers,
    /// More string parameters are present in the file than this crate knows how to interpret.
    TooManyStrings,
    /// The length of some field was not >= -1.
    InvalidLength,
    /// The names table was missing a trailing null terminator.
    NamesMissingNull,
    /// The strings table was missing a trailing null terminator.
    StringsMissingNull,
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        use std::error::Error;
        match self {
            &NotUtf8(e) => write!(f, "{}", e),
            &BadMagic(v) => write!(f, "bad magic number {:x} in terminfo header", v),
            _ => f.write_str(self.description()),
        }
    }
}

impl ::std::convert::From<::std::string::FromUtf8Error> for Error {
    fn from(v: ::std::string::FromUtf8Error) -> Self {
        NotUtf8(v.utf8_error())
    }
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str {
        match self {
            &BadMagic(..) => "incorrect magic number at start of file",
            &ShortNames => "no names exposed, need at least one",
            &TooManyBools => "more boolean properties than libterm knows about",
            &TooManyNumbers => "more number properties than libterm knows about",
            &TooManyStrings => "more string properties than libterm knows about",
            &InvalidLength => "invalid length field value, must be >= -1",
            &NotUtf8(ref e) => e.description(),
            &NamesMissingNull => "names table missing NUL terminator",
            &StringsMissingNull => "string table missing NUL terminator",
        }
    }

    fn cause(&self) -> Option<&::std::error::Error> {
        match self {
            &NotUtf8(ref e) => Some(e),
            _ => None,
        }
    }
}

pub mod searcher;

/// TermInfo format parsing.
pub mod parser {
    //! ncurses-compatible compiled terminfo format parsing (term(5))
    pub mod compiled;
    mod names;
}
pub mod parm;


fn cap_for_attr(attr: Attr) -> &'static str {
    match attr {
        Attr::Bold => "bold",
        Attr::Dim => "dim",
        Attr::Italic(true) => "sitm",
        Attr::Italic(false) => "ritm",
        Attr::Underline(true) => "smul",
        Attr::Underline(false) => "rmul",
        Attr::Blink => "blink",
        Attr::Standout(true) => "smso",
        Attr::Standout(false) => "rmso",
        Attr::Reverse => "rev",
        Attr::Secure => "invis",
        Attr::ForegroundColor(_) => "setaf",
        Attr::BackgroundColor(_) => "setab",
    }
}

/// A Terminal that knows how many colors it supports, with a reference to its
/// parsed Terminfo database record.
pub struct TerminfoTerminal<T> {
    num_colors: u16,
    out: T,
    ti: TermInfo,
}

impl<T: Write + Send> Terminal for TerminfoTerminal<T> {
    type Output = T;
    fn fg(&mut self, color: color::Color) -> Result<()> {
        let color = self.dim_if_necessary(color);
        if self.num_colors > color {
            return self.apply_cap("setaf", &[Param::Number(color as i32)]);
        }
        Err(::Error::ColorOutOfRange)
    }

    fn bg(&mut self, color: color::Color) -> Result<()> {
        let color = self.dim_if_necessary(color);
        if self.num_colors > color {
            return self.apply_cap("setab", &[Param::Number(color as i32)]);
        }
        Err(::Error::ColorOutOfRange)
    }

    fn attr(&mut self, attr: Attr) -> Result<()> {
        match attr {
            Attr::ForegroundColor(c) => self.fg(c),
            Attr::BackgroundColor(c) => self.bg(c),
            _ => self.apply_cap(cap_for_attr(attr), &[]),
        }
    }

    fn supports_attr(&self, attr: Attr) -> bool {
        match attr {
            Attr::ForegroundColor(_) | Attr::BackgroundColor(_) => self.num_colors > 0,
            _ => {
                let cap = cap_for_attr(attr);
                self.ti.strings.get(cap).is_some()
            }
        }
    }

    fn reset(&mut self) -> Result<()> {
        // are there any terminals that have color/attrs and not sgr0?
        // Try falling back to sgr, then op
        let cmd = match [("sgr0", &[] as &[Param]),
                         ("sgr", &[Param::Number(0)]),
                         ("op", &[])]
                            .iter()
                            .filter_map(|&(cap, params)| {
                                self.ti.strings.get(cap).map(|c| (c, params))
                            })
                            .next() {
            Some((op, params)) => {
                match expand(op, params, &mut Variables::new()) {
                    Ok(cmd) => cmd,
                    Err(e) => return Err(e.into()),
                }
            }
            None => return Err(::Error::NotSupported),
        };
        try!(self.out.write_all(&cmd));
        Ok(())
    }

    fn supports_reset(&self) -> bool {
        ["sgr0", "sgr", "op"].iter().any(|&cap| self.ti.strings.get(cap).is_some())
    }

    fn supports_color(&self) -> bool {
        self.num_colors > 0 && self.supports_reset()
    }

    fn cursor_up(&mut self) -> Result<()> {
        self.apply_cap("cuu1", &[])
    }

    fn delete_line(&mut self) -> Result<()> {
        self.apply_cap("dl", &[])
    }

    fn carriage_return(&mut self) -> Result<()> {
        self.apply_cap("cr", &[])
    }

    fn get_ref(&self) -> &T {
        &self.out
    }

    fn get_mut(&mut self) -> &mut T {
        &mut self.out
    }

    fn into_inner(self) -> T
        where Self: Sized
    {
        self.out
    }
}

impl<T: Write + Send> TerminfoTerminal<T> {
    /// Create a new TerminfoTerminal with the given TermInfo and Write.
    pub fn new_with_terminfo(out: T, terminfo: TermInfo) -> TerminfoTerminal<T> {
        let nc = if terminfo.strings.contains_key("setaf") &&
                    terminfo.strings.contains_key("setab") {
            terminfo.numbers.get("colors").map_or(0, |&n| n)
        } else {
            0
        };

        TerminfoTerminal {
            out: out,
            ti: terminfo,
            num_colors: nc,
        }
    }

    /// Create a new TerminfoTerminal for the current environment with the given Write.
    ///
    /// Returns `None` when the terminfo cannot be found or parsed.
    pub fn new(out: T) -> Option<TerminfoTerminal<T>> {
        TermInfo::from_env().map(move |ti| TerminfoTerminal::new_with_terminfo(out, ti)).ok()
    }

    fn dim_if_necessary(&self, color: color::Color) -> color::Color {
        if color >= self.num_colors && color >= 8 && color < 16 {
            color - 8
        } else {
            color
        }
    }

    fn apply_cap(&mut self, cmd: &str, params: &[Param]) -> Result<()> {
        match self.ti.strings.get(cmd) {
            Some(cmd) => {
                match expand(&cmd, params, &mut Variables::new()) {
                    Ok(s) => {
                        try!(self.out.write_all(&s));
                        Ok(())
                    }
                    Err(e) => Err(e.into()),
                }
            }
            None => Err(::Error::NotSupported),
        }
    }
}


impl<T: Write> Write for TerminfoTerminal<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.out.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}
