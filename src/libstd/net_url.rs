// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Types/fns concerning URLs (see RFC 3986)

#[allow(deprecated_mode)];

use core::cmp::Eq;
use core::from_str::FromStr;
use core::io::{Reader, ReaderUtil};
use core::io;
use core::prelude::*;
use core::hashmap::linear::LinearMap;
use core::str;
use core::to_bytes::IterBytes;
use core::to_bytes;
use core::to_str::ToStr;
use core::to_str;
use core::uint;

#[deriving(Eq)]
struct Url {
    scheme: ~str,
    user: Option<UserInfo>,
    host: ~str,
    port: Option<~str>,
    path: ~str,
    query: Query,
    fragment: Option<~str>
}

#[deriving(Eq)]
struct UserInfo {
    user: ~str,
    pass: Option<~str>
}

pub type Query = ~[(~str, ~str)];

pub impl Url {
    fn new(
        scheme: ~str,
        user: Option<UserInfo>,
        host: ~str,
        port: Option<~str>,
        path: ~str,
        query: Query,
        fragment: Option<~str>
    ) -> Url {
        Url {
            scheme: scheme,
            user: user,
            host: host,
            port: port,
            path: path,
            query: query,
            fragment: fragment,
        }
    }
}

pub impl UserInfo {
    fn new(user: ~str, pass: Option<~str>) -> UserInfo {
        UserInfo { user: user, pass: pass }
    }
}

fn encode_inner(s: &str, full_url: bool) -> ~str {
    do io::with_str_reader(s) |rdr| {
        let mut out = ~"";

        while !rdr.eof() {
            let ch = rdr.read_byte() as char;
            match ch {
              // unreserved:
              'A' .. 'Z' |
              'a' .. 'z' |
              '0' .. '9' |
              '-' | '.' | '_' | '~' => {
                str::push_char(&mut out, ch);
              }
              _ => {
                  if full_url {
                    match ch {
                      // gen-delims:
                      ':' | '/' | '?' | '#' | '[' | ']' | '@' |

                      // sub-delims:
                      '!' | '$' | '&' | '"' | '(' | ')' | '*' |
                      '+' | ',' | ';' | '=' => {
                        str::push_char(&mut out, ch);
                      }

                      _ => out += fmt!("%%%X", ch as uint)
                    }
                } else {
                    out += fmt!("%%%X", ch as uint);
                }
              }
            }
        }

        out
    }
}

/**
 * Encodes a URI by replacing reserved characters with percent encoded
 * character sequences.
 *
 * This function is compliant with RFC 3986.
 */
pub fn encode(s: &str) -> ~str {
    // FIXME(#3722): unsafe only because encode_inner does (string) IO
    unsafe {encode_inner(s, true)}
}

/**
 * Encodes a URI component by replacing reserved characters with percent
 * encoded character sequences.
 *
 * This function is compliant with RFC 3986.
 */

pub fn encode_component(s: &str) -> ~str {
    // FIXME(#3722): unsafe only because encode_inner does (string) IO
    unsafe {encode_inner(s, false)}
}

fn decode_inner(s: &str, full_url: bool) -> ~str {
    do io::with_str_reader(s) |rdr| {
        let mut out = ~"";

        while !rdr.eof() {
            match rdr.read_char() {
              '%' => {
                let bytes = rdr.read_bytes(2u);
                let ch = uint::parse_bytes(bytes, 16u).get() as char;

                if full_url {
                    // Only decode some characters:
                    match ch {
                      // gen-delims:
                      ':' | '/' | '?' | '#' | '[' | ']' | '@' |

                      // sub-delims:
                      '!' | '$' | '&' | '"' | '(' | ')' | '*' |
                      '+' | ',' | ';' | '=' => {
                        str::push_char(&mut out, '%');
                        str::push_char(&mut out, bytes[0u] as char);
                        str::push_char(&mut out, bytes[1u] as char);
                      }

                      ch => str::push_char(&mut out, ch)
                    }
                } else {
                      str::push_char(&mut out, ch);
                }
              }
              ch => str::push_char(&mut out, ch)
            }
        }

        out
    }
}

/**
 * Decode a string encoded with percent encoding.
 *
 * This will only decode escape sequences generated by encode.
 */
pub fn decode(s: &str) -> ~str {
    // FIXME(#3722): unsafe only because decode_inner does (string) IO
    unsafe {decode_inner(s, true)}
}

/**
 * Decode a string encoded with percent encoding.
 */
pub fn decode_component(s: &str) -> ~str {
    // FIXME(#3722): unsafe only because decode_inner does (string) IO
    unsafe {decode_inner(s, false)}
}

fn encode_plus(s: &str) -> ~str {
    do io::with_str_reader(s) |rdr| {
        let mut out = ~"";

        while !rdr.eof() {
            let ch = rdr.read_byte() as char;
            match ch {
              'A' .. 'Z' | 'a' .. 'z' | '0' .. '9' | '_' | '.' | '-' => {
                str::push_char(&mut out, ch);
              }
              ' ' => str::push_char(&mut out, '+'),
              _ => out += fmt!("%%%X", ch as uint)
            }
        }

        out
    }
}

/**
 * Encode a hashmap to the 'application/x-www-form-urlencoded' media type.
 */
pub fn encode_form_urlencoded(m: &LinearMap<~str, ~[~str]>) -> ~str {
    let mut out = ~"";
    let mut first = true;

    for m.each |&(key, values)| {
        let key = encode_plus(*key);

        for values.each |value| {
            if first {
                first = false;
            } else {
                str::push_char(&mut out, '&');
                first = false;
            }

            out += fmt!("%s=%s", key, encode_plus(*value));
        }
    }

    out
}

/**
 * Decode a string encoded with the 'application/x-www-form-urlencoded' media
 * type into a hashmap.
 */
pub fn decode_form_urlencoded(s: &[u8]) -> LinearMap<~str, ~[~str]> {
    do io::with_bytes_reader(s) |rdr| {
        let mut m = LinearMap::new();
        let mut key = ~"";
        let mut value = ~"";
        let mut parsing_key = true;

        while !rdr.eof() {
            match rdr.read_char() {
                '&' | ';' => {
                    if key != ~"" && value != ~"" {
                        let mut values = match m.pop(&key) {
                            Some(values) => values,
                            None => ~[],
                        };

                        values.push(value);
                        m.insert(key, values);
                    }

                    parsing_key = true;
                    key = ~"";
                    value = ~"";
                }
                '=' => parsing_key = false,
                ch => {
                    let ch = match ch {
                        '%' => {
                            let bytes = rdr.read_bytes(2u);
                            uint::parse_bytes(bytes, 16u).get() as char
                        }
                        '+' => ' ',
                        ch => ch
                    };

                    if parsing_key {
                        str::push_char(&mut key, ch)
                    } else {
                        str::push_char(&mut value, ch)
                    }
                }
            }
        }

        if key != ~"" && value != ~"" {
            let mut values = match m.pop(&key) {
                Some(values) => values,
                None => ~[],
            };

            values.push(value);
            m.insert(key, values);
        }

        m
    }
}


fn split_char_first(s: &str, c: char) -> (~str, ~str) {
    let len = str::len(s);
    let mut index = len;
    let mut mat = 0;
    // FIXME(#3722): unsafe only because decode_inner does (string) IO
    unsafe {
        do io::with_str_reader(s) |rdr| {
            let mut ch;
            while !rdr.eof() {
                ch = rdr.read_byte() as char;
                if ch == c {
                    // found a match, adjust markers
                    index = rdr.tell()-1;
                    mat = 1;
                    break;
                }
            }
        }
    }
    if index+mat == len {
        return (str::slice(s, 0, index).to_owned(), ~"");
    } else {
        return (str::slice(s, 0, index).to_owned(),
             str::slice(s, index + mat, str::len(s)).to_owned());
    }
}

fn userinfo_from_str(uinfo: &str) -> UserInfo {
    let (user, p) = split_char_first(uinfo, ':');
    let pass = if str::len(p) == 0 {
        None
    } else {
        Some(p)
    };
    return UserInfo::new(user, pass);
}

fn userinfo_to_str(userinfo: &UserInfo) -> ~str {
    match userinfo.pass {
        Some(ref pass) => fmt!("%s:%s@", userinfo.user, *pass),
        None => fmt!("%s@", userinfo.user),
    }
}

fn query_from_str(rawquery: &str) -> Query {
    let mut query: Query = ~[];
    if str::len(rawquery) != 0 {
        for str::split_char(rawquery, '&').each |p| {
            let (k, v) = split_char_first(*p, '=');
            // FIXME(#3722): unsafe only because decode_inner does (string) IO
            unsafe {query.push((decode_component(k), decode_component(v)));}
        };
    }
    return query;
}

pub fn query_to_str(query: &Query) -> ~str {
    unsafe {
        // FIXME(#3722): unsafe only because decode_inner does (string) IO
        let mut strvec = ~[];
        for query.each |kv| {
            match kv {
                &(ref k, ref v) => {
                    strvec.push(fmt!("%s=%s",
                        encode_component(*k),
                        encode_component(*v))
                    );
                }
            }
        }
        return str::connect(strvec, ~"&");
    }
}

// returns the scheme and the rest of the url, or a parsing error
pub fn get_scheme(rawurl: &str) -> Result<(~str, ~str), ~str> {
    for str::each_chari(rawurl) |i,c| {
        match c {
          'A' .. 'Z' | 'a' .. 'z' => loop,
          '0' .. '9' | '+' | '-' | '.' => {
            if i == 0 {
                return Err(~"url: Scheme must begin with a letter.");
            }
            loop;
          }
          ':' => {
            if i == 0 {
                return Err(~"url: Scheme cannot be empty.");
            } else {
                return Ok((rawurl.slice(0,i).to_owned(),
                                rawurl.slice(i+1,str::len(rawurl)).to_owned()));
            }
          }
          _ => {
            return Err(~"url: Invalid character in scheme.");
          }
        }
    };
    return Err(~"url: Scheme must be terminated with a colon.");
}

#[deriving(Eq)]
enum Input {
    Digit, // all digits
    Hex, // digits and letters a-f
    Unreserved // all other legal characters
}

// returns userinfo, host, port, and unparsed part, or an error
fn get_authority(rawurl: &str) ->
    Result<(Option<UserInfo>, ~str, Option<~str>, ~str), ~str> {
    if !str::starts_with(rawurl, ~"//") {
        // there is no authority.
        return Ok((None, ~"", None, rawurl.to_str()));
    }

    enum State {
        Start, // starting state
        PassHostPort, // could be in user or port
        Ip6Port, // either in ipv6 host or port
        Ip6Host, // are in an ipv6 host
        InHost, // are in a host - may be ipv6, but don't know yet
        InPort // are in port
    }

    let len = rawurl.len();
    let mut st = Start;
    let mut in = Digit; // most restricted, start here.

    let mut userinfo = None;
    let mut host = ~"";
    let mut port = None;

    let mut colon_count = 0;
    let mut pos = 0, begin = 2, end = len;

    for str::each_chari(rawurl) |i,c| {
        if i < 2 { loop; } // ignore the leading //

        // deal with input class first
        match c {
          '0' .. '9' => (),
          'A' .. 'F' | 'a' .. 'f' => {
            if in == Digit {
                in = Hex;
            }
          }
          'G' .. 'Z' | 'g' .. 'z' | '-' | '.' | '_' | '~' | '%' |
          '&' |'\'' | '(' | ')' | '+' | '!' | '*' | ',' | ';' | '=' => {
            in = Unreserved;
          }
          ':' | '@' | '?' | '#' | '/' => {
            // separators, don't change anything
          }
          _ => {
            return Err(~"Illegal character in authority");
          }
        }

        // now process states
        match c {
          ':' => {
            colon_count += 1;
            match st {
              Start => {
                pos = i;
                st = PassHostPort;
              }
              PassHostPort => {
                // multiple colons means ipv6 address.
                if in == Unreserved {
                    return Err(
                        ~"Illegal characters in IPv6 address.");
                }
                st = Ip6Host;
              }
              InHost => {
                pos = i;
                // can't be sure whether this is an ipv6 address or a port
                if in == Unreserved {
                    return Err(~"Illegal characters in authority.");
                }
                st = Ip6Port;
              }
              Ip6Port => {
                if in == Unreserved {
                    return Err(~"Illegal characters in authority.");
                }
                st = Ip6Host;
              }
              Ip6Host => {
                if colon_count > 7 {
                    host = str::slice(rawurl, begin, i).to_owned();
                    pos = i;
                    st = InPort;
                }
              }
              _ => {
                return Err(~"Invalid ':' in authority.");
              }
            }
            in = Digit; // reset input class
          }

          '@' => {
            in = Digit; // reset input class
            colon_count = 0; // reset count
            match st {
              Start => {
                let user = str::slice(rawurl, begin, i).to_owned();
                userinfo = Some(UserInfo::new(user, None));
                st = InHost;
              }
              PassHostPort => {
                let user = str::slice(rawurl, begin, pos).to_owned();
                let pass = str::slice(rawurl, pos+1, i).to_owned();
                userinfo = Some(UserInfo::new(user, Some(pass)));
                st = InHost;
              }
              _ => {
                return Err(~"Invalid '@' in authority.");
              }
            }
            begin = i+1;
          }

          '?' | '#' | '/' => {
            end = i;
            break;
          }
          _ => ()
        }
        end = i;
    }

    let end = end; // make end immutable so it can be captured

    let host_is_end_plus_one: &fn() -> bool = || {
        end+1 == len
            && !['?', '#', '/'].contains(&(rawurl[end] as char))
    };

    // finish up
    match st {
      Start => {
        if host_is_end_plus_one() {
            host = str::slice(rawurl, begin, end+1).to_owned();
        } else {
            host = str::slice(rawurl, begin, end).to_owned();
        }
      }
      PassHostPort | Ip6Port => {
        if in != Digit {
            return Err(~"Non-digit characters in port.");
        }
        host = str::slice(rawurl, begin, pos).to_owned();
        port = Some(str::slice(rawurl, pos+1, end).to_owned());
      }
      Ip6Host | InHost => {
        host = str::slice(rawurl, begin, end).to_owned();
      }
      InPort => {
        if in != Digit {
            return Err(~"Non-digit characters in port.");
        }
        port = Some(str::slice(rawurl, pos+1, end).to_owned());
      }
    }

    let rest = if host_is_end_plus_one() { ~"" }
    else { str::slice(rawurl, end, len).to_owned() };
    return Ok((userinfo, host, port, rest));
}


// returns the path and unparsed part of url, or an error
fn get_path(rawurl: &str, authority: bool) ->
    Result<(~str, ~str), ~str> {
    let len = str::len(rawurl);
    let mut end = len;
    for str::each_chari(rawurl) |i,c| {
        match c {
          'A' .. 'Z' | 'a' .. 'z' | '0' .. '9' | '&' |'\'' | '(' | ')' | '.'
          | '@' | ':' | '%' | '/' | '+' | '!' | '*' | ',' | ';' | '='
          | '_' | '-' => {
            loop;
          }
          '?' | '#' => {
            end = i;
            break;
          }
          _ => return Err(~"Invalid character in path.")
        }
    }

    if authority {
        if end != 0 && !str::starts_with(rawurl, ~"/") {
            return Err(~"Non-empty path must begin with\
                               '/' in presence of authority.");
        }
    }

    return Ok((decode_component(str::slice(rawurl, 0, end).to_owned()),
                    str::slice(rawurl, end, len).to_owned()));
}

// returns the parsed query and the fragment, if present
fn get_query_fragment(rawurl: &str) ->
    Result<(Query, Option<~str>), ~str> {
    if !str::starts_with(rawurl, ~"?") {
        if str::starts_with(rawurl, ~"#") {
            let f = decode_component(str::slice(rawurl,
                                                1,
                                                str::len(rawurl)).to_owned());
            return Ok((~[], Some(f)));
        } else {
            return Ok((~[], None));
        }
    }
    let (q, r) = split_char_first(str::slice(rawurl, 1,
                                             str::len(rawurl)).to_owned(), '#');
    let f = if str::len(r) != 0 {
        Some(decode_component(r)) } else { None };
    return Ok((query_from_str(q), f));
}

/**
 * Parse a `str` to a `url`
 *
 * # Arguments
 *
 * `rawurl` - a string representing a full url, including scheme.
 *
 * # Returns
 *
 * a `url` that contains the parsed representation of the url.
 *
 */

pub fn from_str(rawurl: &str) -> Result<Url, ~str> {
    // scheme
    let (scheme, rest) = match get_scheme(rawurl) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };

    // authority
    let (userinfo, host, port, rest) = match get_authority(rest) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };

    // path
    let has_authority = if host == ~"" { false } else { true };
    let (path, rest) = match get_path(rest, has_authority) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };

    // query and fragment
    let (query, fragment) = match get_query_fragment(rest) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };

    Ok(Url::new(scheme, userinfo, host, port, path, query, fragment))
}

impl FromStr for Url {
    fn from_str(s: &str) -> Option<Url> {
        match from_str(s) {
            Ok(url) => Some(url),
            Err(_) => None
        }
    }
}

/**
 * Format a `url` as a string
 *
 * # Arguments
 *
 * `url` - a url.
 *
 * # Returns
 *
 * a `str` that contains the formatted url. Note that this will usually
 * be an inverse of `from_str` but might strip out unneeded separators.
 * for example, "http://somehost.com?", when parsed and formatted, will
 * result in just "http://somehost.com".
 *
 */
pub fn to_str(url: &Url) -> ~str {
    let user = match url.user {
        Some(ref user) => userinfo_to_str(user),
        None => ~"",
    };

    let authority = if url.host.is_empty() {
        ~""
    } else {
        fmt!("//%s%s", user, url.host)
    };

    let query = if url.query.is_empty() {
        ~""
    } else {
        fmt!("?%s", query_to_str(&url.query))
    };

    let fragment = match url.fragment {
        Some(ref fragment) => fmt!("#%s", encode_component(*fragment)),
        None => ~"",
    };

    fmt!("%s:%s%s%s%s", url.scheme, authority, url.path, query, fragment)
}

impl to_str::ToStr for Url {
    pub fn to_str(&self) -> ~str {
        to_str(self)
    }
}

impl to_bytes::IterBytes for Url {
    fn iter_bytes(&self, lsb0: bool, f: to_bytes::Cb) {
        self.to_str().iter_bytes(lsb0, f)
    }
}

// Put a few tests outside of the 'test' module so they can test the internal
// functions and those functions don't need 'pub'

#[test]
fn test_split_char_first() {
    let (u,v) = split_char_first(~"hello, sweet world", ',');
    fail_unless!(u == ~"hello");
    fail_unless!(v == ~" sweet world");

    let (u,v) = split_char_first(~"hello sweet world", ',');
    fail_unless!(u == ~"hello sweet world");
    fail_unless!(v == ~"");
}

#[test]
fn test_get_authority() {
    let (u, h, p, r) = get_authority(
        "//user:pass@rust-lang.org/something").unwrap();
    fail_unless!(u == Some(UserInfo::new(~"user", Some(~"pass"))));
    fail_unless!(h == ~"rust-lang.org");
    fail_unless!(p.is_none());
    fail_unless!(r == ~"/something");

    let (u, h, p, r) = get_authority(
        "//rust-lang.org:8000?something").unwrap();
    fail_unless!(u.is_none());
    fail_unless!(h == ~"rust-lang.org");
    fail_unless!(p == Some(~"8000"));
    fail_unless!(r == ~"?something");

    let (u, h, p, r) = get_authority(
        "//rust-lang.org#blah").unwrap();
    fail_unless!(u.is_none());
    fail_unless!(h == ~"rust-lang.org");
    fail_unless!(p.is_none());
    fail_unless!(r == ~"#blah");

    // ipv6 tests
    let (_, h, _, _) = get_authority(
        "//2001:0db8:85a3:0042:0000:8a2e:0370:7334#blah").unwrap();
    fail_unless!(h == ~"2001:0db8:85a3:0042:0000:8a2e:0370:7334");

    let (_, h, p, _) = get_authority(
        "//2001:0db8:85a3:0042:0000:8a2e:0370:7334:8000#blah").unwrap();
    fail_unless!(h == ~"2001:0db8:85a3:0042:0000:8a2e:0370:7334");
    fail_unless!(p == Some(~"8000"));

    let (u, h, p, _) = get_authority(
        "//us:p@2001:0db8:85a3:0042:0000:8a2e:0370:7334:8000#blah"
    ).unwrap();
    fail_unless!(u == Some(UserInfo::new(~"us", Some(~"p"))));
    fail_unless!(h == ~"2001:0db8:85a3:0042:0000:8a2e:0370:7334");
    fail_unless!(p == Some(~"8000"));

    // invalid authorities;
    fail_unless!(get_authority("//user:pass@rust-lang:something").is_err());
    fail_unless!(get_authority("//user@rust-lang:something:/path").is_err());
    fail_unless!(get_authority(
        "//2001:0db8:85a3:0042:0000:8a2e:0370:7334:800a").is_err());
    fail_unless!(get_authority(
        "//2001:0db8:85a3:0042:0000:8a2e:0370:7334:8000:00").is_err());

    // these parse as empty, because they don't start with '//'
    let (_, h, _, _) = get_authority(~"user:pass@rust-lang").unwrap();
    fail_unless!(h == ~"");
    let (_, h, _, _) = get_authority(~"rust-lang.org").unwrap();
    fail_unless!(h == ~"");
}

#[test]
fn test_get_path() {
    let (p, r) = get_path("/something+%20orother", true).unwrap();
    fail_unless!(p == ~"/something+ orother");
    fail_unless!(r == ~"");
    let (p, r) = get_path("test@email.com#fragment", false).unwrap();
    fail_unless!(p == ~"test@email.com");
    fail_unless!(r == ~"#fragment");
    let (p, r) = get_path(~"/gen/:addr=?q=v", false).unwrap();
    fail_unless!(p == ~"/gen/:addr=");
    fail_unless!(r == ~"?q=v");

    //failure cases
    fail_unless!(get_path(~"something?q", true).is_err());
}

#[cfg(test)]
mod tests {
    use core::prelude::*;

    use net_url::*;

    use core::hashmap::linear::LinearMap;

    #[test]
    pub fn test_url_parse() {
        let url = ~"http://user:pass@rust-lang.org/doc?s=v#something";

        let up = from_str(url);
        let u = up.unwrap();
        fail_unless!(u.scheme == ~"http");
        let userinfo = u.user.get_ref();
        fail_unless!(userinfo.user == ~"user");
        fail_unless!(userinfo.pass.get_ref() == &~"pass");
        fail_unless!(u.host == ~"rust-lang.org");
        fail_unless!(u.path == ~"/doc");
        fail_unless!(u.query == ~[(~"s", ~"v")]);
        fail_unless!(u.fragment.get_ref() == &~"something");
    }

    #[test]
    pub fn test_url_parse_host_slash() {
        let urlstr = ~"http://0.42.42.42/";
        let url = from_str(urlstr).unwrap();
        fail_unless!(url.host == ~"0.42.42.42");
        fail_unless!(url.path == ~"/");
    }

    #[test]
    pub fn test_url_with_underscores() {
        let urlstr = ~"http://dotcom.com/file_name.html";
        let url = from_str(urlstr).unwrap();
        fail_unless!(url.path == ~"/file_name.html");
    }

    #[test]
    pub fn test_url_with_dashes() {
        let urlstr = ~"http://dotcom.com/file-name.html";
        let url = from_str(urlstr).unwrap();
        fail_unless!(url.path == ~"/file-name.html");
    }

    #[test]
    pub fn test_no_scheme() {
        fail_unless!(get_scheme("noschemehere.html").is_err());
    }

    #[test]
    pub fn test_invalid_scheme_errors() {
        fail_unless!(from_str("99://something").is_err());
        fail_unless!(from_str("://something").is_err());
    }

    #[test]
    pub fn test_full_url_parse_and_format() {
        let url = ~"http://user:pass@rust-lang.org/doc?s=v#something";
        fail_unless!(from_str(url).unwrap().to_str() == url);
    }

    #[test]
    pub fn test_userless_url_parse_and_format() {
        let url = ~"http://rust-lang.org/doc?s=v#something";
        fail_unless!(from_str(url).unwrap().to_str() == url);
    }

    #[test]
    pub fn test_queryless_url_parse_and_format() {
        let url = ~"http://user:pass@rust-lang.org/doc#something";
        fail_unless!(from_str(url).unwrap().to_str() == url);
    }

    #[test]
    pub fn test_empty_query_url_parse_and_format() {
        let url = ~"http://user:pass@rust-lang.org/doc?#something";
        let should_be = ~"http://user:pass@rust-lang.org/doc#something";
        fail_unless!(from_str(url).unwrap().to_str() == should_be);
    }

    #[test]
    pub fn test_fragmentless_url_parse_and_format() {
        let url = ~"http://user:pass@rust-lang.org/doc?q=v";
        fail_unless!(from_str(url).unwrap().to_str() == url);
    }

    #[test]
    pub fn test_minimal_url_parse_and_format() {
        let url = ~"http://rust-lang.org/doc";
        fail_unless!(from_str(url).unwrap().to_str() == url);
    }

    #[test]
    pub fn test_scheme_host_only_url_parse_and_format() {
        let url = ~"http://rust-lang.org";
        fail_unless!(from_str(url).unwrap().to_str() == url);
    }

    #[test]
    pub fn test_pathless_url_parse_and_format() {
        let url = ~"http://user:pass@rust-lang.org?q=v#something";
        fail_unless!(from_str(url).unwrap().to_str() == url);
    }

    #[test]
    pub fn test_scheme_host_fragment_only_url_parse_and_format() {
        let url = ~"http://rust-lang.org#something";
        fail_unless!(from_str(url).unwrap().to_str() == url);
    }

    #[test]
    pub fn test_url_component_encoding() {
        let url = ~"http://rust-lang.org/doc%20uments?ba%25d%20=%23%26%2B";
        let u = from_str(url).unwrap();
        fail_unless!(u.path == ~"/doc uments");
        fail_unless!(u.query == ~[(~"ba%d ", ~"#&+")]);
    }

    #[test]
    pub fn test_url_without_authority() {
        let url = ~"mailto:test@email.com";
        fail_unless!(from_str(url).unwrap().to_str() == url);
    }

    #[test]
    pub fn test_encode() {
        fail_unless!(encode("") == ~"");
        fail_unless!(encode("http://example.com") == ~"http://example.com");
        fail_unless!(encode("foo bar% baz") == ~"foo%20bar%25%20baz");
        fail_unless!(encode(" ") == ~"%20");
        fail_unless!(encode("!") == ~"!");
        fail_unless!(encode("\"") == ~"\"");
        fail_unless!(encode("#") == ~"#");
        fail_unless!(encode("$") == ~"$");
        fail_unless!(encode("%") == ~"%25");
        fail_unless!(encode("&") == ~"&");
        fail_unless!(encode("'") == ~"%27");
        fail_unless!(encode("(") == ~"(");
        fail_unless!(encode(")") == ~")");
        fail_unless!(encode("*") == ~"*");
        fail_unless!(encode("+") == ~"+");
        fail_unless!(encode(",") == ~",");
        fail_unless!(encode("/") == ~"/");
        fail_unless!(encode(":") == ~":");
        fail_unless!(encode(";") == ~";");
        fail_unless!(encode("=") == ~"=");
        fail_unless!(encode("?") == ~"?");
        fail_unless!(encode("@") == ~"@");
        fail_unless!(encode("[") == ~"[");
        fail_unless!(encode("]") == ~"]");
    }

    #[test]
    pub fn test_encode_component() {
        fail_unless!(encode_component("") == ~"");
        fail_unless!(encode_component("http://example.com") ==
            ~"http%3A%2F%2Fexample.com");
        fail_unless!(encode_component("foo bar% baz") ==
            ~"foo%20bar%25%20baz");
        fail_unless!(encode_component(" ") == ~"%20");
        fail_unless!(encode_component("!") == ~"%21");
        fail_unless!(encode_component("#") == ~"%23");
        fail_unless!(encode_component("$") == ~"%24");
        fail_unless!(encode_component("%") == ~"%25");
        fail_unless!(encode_component("&") == ~"%26");
        fail_unless!(encode_component("'") == ~"%27");
        fail_unless!(encode_component("(") == ~"%28");
        fail_unless!(encode_component(")") == ~"%29");
        fail_unless!(encode_component("*") == ~"%2A");
        fail_unless!(encode_component("+") == ~"%2B");
        fail_unless!(encode_component(",") == ~"%2C");
        fail_unless!(encode_component("/") == ~"%2F");
        fail_unless!(encode_component(":") == ~"%3A");
        fail_unless!(encode_component(";") == ~"%3B");
        fail_unless!(encode_component("=") == ~"%3D");
        fail_unless!(encode_component("?") == ~"%3F");
        fail_unless!(encode_component("@") == ~"%40");
        fail_unless!(encode_component("[") == ~"%5B");
        fail_unless!(encode_component("]") == ~"%5D");
    }

    #[test]
    pub fn test_decode() {
        fail_unless!(decode("") == ~"");
        fail_unless!(decode("abc/def 123") == ~"abc/def 123");
        fail_unless!(decode("abc%2Fdef%20123") == ~"abc%2Fdef 123");
        fail_unless!(decode("%20") == ~" ");
        fail_unless!(decode("%21") == ~"%21");
        fail_unless!(decode("%22") == ~"%22");
        fail_unless!(decode("%23") == ~"%23");
        fail_unless!(decode("%24") == ~"%24");
        fail_unless!(decode("%25") == ~"%");
        fail_unless!(decode("%26") == ~"%26");
        fail_unless!(decode("%27") == ~"'");
        fail_unless!(decode("%28") == ~"%28");
        fail_unless!(decode("%29") == ~"%29");
        fail_unless!(decode("%2A") == ~"%2A");
        fail_unless!(decode("%2B") == ~"%2B");
        fail_unless!(decode("%2C") == ~"%2C");
        fail_unless!(decode("%2F") == ~"%2F");
        fail_unless!(decode("%3A") == ~"%3A");
        fail_unless!(decode("%3B") == ~"%3B");
        fail_unless!(decode("%3D") == ~"%3D");
        fail_unless!(decode("%3F") == ~"%3F");
        fail_unless!(decode("%40") == ~"%40");
        fail_unless!(decode("%5B") == ~"%5B");
        fail_unless!(decode("%5D") == ~"%5D");
    }

    #[test]
    pub fn test_decode_component() {
        fail_unless!(decode_component("") == ~"");
        fail_unless!(decode_component("abc/def 123") == ~"abc/def 123");
        fail_unless!(decode_component("abc%2Fdef%20123") == ~"abc/def 123");
        fail_unless!(decode_component("%20") == ~" ");
        fail_unless!(decode_component("%21") == ~"!");
        fail_unless!(decode_component("%22") == ~"\"");
        fail_unless!(decode_component("%23") == ~"#");
        fail_unless!(decode_component("%24") == ~"$");
        fail_unless!(decode_component("%25") == ~"%");
        fail_unless!(decode_component("%26") == ~"&");
        fail_unless!(decode_component("%27") == ~"'");
        fail_unless!(decode_component("%28") == ~"(");
        fail_unless!(decode_component("%29") == ~")");
        fail_unless!(decode_component("%2A") == ~"*");
        fail_unless!(decode_component("%2B") == ~"+");
        fail_unless!(decode_component("%2C") == ~",");
        fail_unless!(decode_component("%2F") == ~"/");
        fail_unless!(decode_component("%3A") == ~":");
        fail_unless!(decode_component("%3B") == ~";");
        fail_unless!(decode_component("%3D") == ~"=");
        fail_unless!(decode_component("%3F") == ~"?");
        fail_unless!(decode_component("%40") == ~"@");
        fail_unless!(decode_component("%5B") == ~"[");
        fail_unless!(decode_component("%5D") == ~"]");
    }

    #[test]
    pub fn test_encode_form_urlencoded() {
        let mut m = LinearMap::new();
        fail_unless!(encode_form_urlencoded(&m) == ~"");

        m.insert(~"", ~[]);
        m.insert(~"foo", ~[]);
        fail_unless!(encode_form_urlencoded(&m) == ~"");

        let mut m = LinearMap::new();
        m.insert(~"foo", ~[~"bar", ~"123"]);
        fail_unless!(encode_form_urlencoded(&m) == ~"foo=bar&foo=123");

        let mut m = LinearMap::new();
        m.insert(~"foo bar", ~[~"abc", ~"12 = 34"]);
        fail_unless!(encode_form_urlencoded(&m) ==
            ~"foo+bar=abc&foo+bar=12+%3D+34");
    }

    #[test]
    pub fn test_decode_form_urlencoded() {
        // FIXME #4449: Commented out because this causes an ICE, but only
        // on FreeBSD
        /*
        fail_unless!(decode_form_urlencoded(~[]).len() == 0);

        let s = str::to_bytes("a=1&foo+bar=abc&foo+bar=12+%3D+34");
        let form = decode_form_urlencoded(s);
        fail_unless!(form.len() == 2);
        fail_unless!(form.get_ref(&~"a") == &~[~"1"]);
        fail_unless!(form.get_ref(&~"foo bar") == &~[~"abc", ~"12 = 34"]);
        */
    }
}
