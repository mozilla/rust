// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Synchronous DNS Resolution
//!
//! Contains the functionality to perform DNS resolution in a style related to
//! `getaddrinfo()`

#![allow(missing_docs)]

pub use self::SocketType::*;
pub use self::Flag::*;
pub use self::Protocol::*;

use iter::IteratorExt;
use io::{IoResult};
use io::net::ip::{SocketAddr, IpAddr};
use kinds::Copy;
use option::{Option, Some, None};
use sys;
use vec::Vec;

/// Hints to the types of sockets that are desired when looking up hosts
pub enum SocketType {
    Stream, Datagram, Raw
}

impl Copy for SocketType {}

/// Flags which can be or'd into the `flags` field of a `Hint`. These are used
/// to manipulate how a query is performed.
///
/// The meaning of each of these flags can be found with `man -s 3 getaddrinfo`
pub enum Flag {
    AddrConfig,
    All,
    CanonName,
    NumericHost,
    NumericServ,
    Passive,
    V4Mapped,
}

impl Copy for Flag {}

/// A transport protocol associated with either a hint or a return value of
/// `lookup`
pub enum Protocol {
    TCP, UDP
}

impl Copy for Protocol {}

/// This structure is used to provide hints when fetching addresses for a
/// remote host to control how the lookup is performed.
///
/// For details on these fields, see their corresponding definitions via
/// `man -s 3 getaddrinfo`
pub struct Hint {
    pub family: uint,
    pub socktype: Option<SocketType>,
    pub protocol: Option<Protocol>,
    pub flags: uint,
}

impl Copy for Hint {}

pub struct Info {
    pub address: SocketAddr,
    pub family: uint,
    pub socktype: Option<SocketType>,
    pub protocol: Option<Protocol>,
    pub flags: uint,
}

impl Copy for Info {}

/// Easy name resolution. Given a hostname, returns the list of IP addresses for
/// that hostname.
pub fn get_host_addresses(host: &str) -> IoResult<Vec<IpAddr>> {
    lookup(Some(host), None, None).map(|a| a.into_iter().map(|i| i.address.ip).collect())
}

/// Full-fledged resolution. This function will perform a synchronous call to
/// getaddrinfo, controlled by the parameters
///
/// # Arguments
///
/// * hostname - an optional hostname to lookup against
/// * servname - an optional service name, listed in the system services
/// * hint - see the hint structure, and "man -s 3 getaddrinfo", for how this
///          controls lookup
///
/// FIXME: this is not public because the `Hint` structure is not ready for public
///      consumption just yet.
#[allow(unused_variables)]
fn lookup(hostname: Option<&str>, servname: Option<&str>, hint: Option<Hint>)
          -> IoResult<Vec<Info>> {
    sys::addrinfo::get_host_addresses(hostname, servname, hint)
}

// Ignored on android since we cannot give tcp/ip
// permission without help of apk
#[cfg(all(test, not(target_os = "android")))]
mod test {
    use prelude::*;
    use super::*;
    use io::net::ip::*;

    #[test]
    fn dns_smoke_test() {
        let ipaddrs = get_host_addresses("localhost").unwrap();
        let mut found_local = false;
        let local_addr = &Ipv4Addr(127, 0, 0, 1);
        for addr in ipaddrs.iter() {
            found_local = found_local || addr == local_addr;
        }
        assert!(found_local);
    }

    #[ignore]
    #[test]
    fn issue_10663() {
        // Something should happen here, but this certainly shouldn't cause
        // everything to die. The actual outcome we don't care too much about.
        get_host_addresses("example.com").unwrap();
    }
}
