// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use fmt;
use io;
use net::{SocketAddr, Shutdown, Ipv4Addr, Ipv6Addr};
use time::Duration;
use sys::{unsupported, Void};

pub struct TcpStream(Void);

impl TcpStream {
    pub fn connect(_: &SocketAddr) -> io::Result<TcpStream> {
        unsupported()
    }

    pub fn connect_timeout(_: &SocketAddr, _: Duration) -> io::Result<TcpStream> {
        unsupported()
    }

    pub fn set_read_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        match self.0 {}
    }

    pub fn set_write_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        match self.0 {}
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        match self.0 {}
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        match self.0 {}
    }

    pub fn peek(&self, _: &mut [u8]) -> io::Result<usize> {
        match self.0 {}
    }

    pub fn read(&self, _: &mut [u8]) -> io::Result<usize> {
        match self.0 {}
    }

    pub fn write(&self, _: &[u8]) -> io::Result<usize> {
        match self.0 {}
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        match self.0 {}
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        match self.0 {}
    }

    pub fn shutdown(&self, _: Shutdown) -> io::Result<()> {
        match self.0 {}
    }

    pub fn duplicate(&self) -> io::Result<TcpStream> {
        match self.0 {}
    }

    pub fn set_nodelay(&self, _: bool) -> io::Result<()> {
        match self.0 {}
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        match self.0 {}
    }

    pub fn set_ttl(&self, _: u32) -> io::Result<()> {
        match self.0 {}
    }

    pub fn ttl(&self) -> io::Result<u32> {
        match self.0 {}
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        match self.0 {}
    }

    pub fn set_nonblocking(&self, _: bool) -> io::Result<()> {
        match self.0 {}
    }
}

impl fmt::Debug for TcpStream {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {}
    }
}

pub struct TcpListener(Void);

impl TcpListener {
    pub fn bind(_: &SocketAddr) -> io::Result<TcpListener> {
        unsupported()
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        match self.0 {}
    }

    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        match self.0 {}
    }

    pub fn duplicate(&self) -> io::Result<TcpListener> {
        match self.0 {}
    }

    pub fn set_ttl(&self, _: u32) -> io::Result<()> {
        match self.0 {}
    }

    pub fn ttl(&self) -> io::Result<u32> {
        match self.0 {}
    }

    pub fn set_only_v6(&self, _: bool) -> io::Result<()> {
        match self.0 {}
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        match self.0 {}
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        match self.0 {}
    }

    pub fn set_nonblocking(&self, _: bool) -> io::Result<()> {
        match self.0 {}
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {}
    }
}

pub struct UdpSocket(Void);

impl UdpSocket {
    pub fn bind(_: &SocketAddr) -> io::Result<UdpSocket> {
        unsupported()
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        match self.0 {}
    }

    pub fn recv_from(&self, _: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        match self.0 {}
    }

    pub fn peek_from(&self, _: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        match self.0 {}
    }

    pub fn send_to(&self, _: &[u8], _: &SocketAddr) -> io::Result<usize> {
        match self.0 {}
    }

    pub fn duplicate(&self) -> io::Result<UdpSocket> {
        match self.0 {}
    }

    pub fn set_read_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        match self.0 {}
    }

    pub fn set_write_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        match self.0 {}
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        match self.0 {}
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        match self.0 {}
    }

    pub fn set_broadcast(&self, _: bool) -> io::Result<()> {
        match self.0 {}
    }

    pub fn broadcast(&self) -> io::Result<bool> {
        match self.0 {}
    }

    pub fn set_multicast_loop_v4(&self, _: bool) -> io::Result<()> {
        match self.0 {}
    }

    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        match self.0 {}
    }

    pub fn set_multicast_ttl_v4(&self, _: u32) -> io::Result<()> {
        match self.0 {}
    }

    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        match self.0 {}
    }

    pub fn set_multicast_loop_v6(&self, _: bool) -> io::Result<()> {
        match self.0 {}
    }

    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        match self.0 {}
    }

    pub fn join_multicast_v4(&self, _: &Ipv4Addr, _: &Ipv4Addr)
                         -> io::Result<()> {
        match self.0 {}
    }

    pub fn join_multicast_v6(&self, _: &Ipv6Addr, _: u32)
                         -> io::Result<()> {
        match self.0 {}
    }

    pub fn leave_multicast_v4(&self, _: &Ipv4Addr, _: &Ipv4Addr)
                          -> io::Result<()> {
        match self.0 {}
    }

    pub fn leave_multicast_v6(&self, _: &Ipv6Addr, _: u32)
                          -> io::Result<()> {
        match self.0 {}
    }

    pub fn set_ttl(&self, _: u32) -> io::Result<()> {
        match self.0 {}
    }

    pub fn ttl(&self) -> io::Result<u32> {
        match self.0 {}
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        match self.0 {}
    }

    pub fn set_nonblocking(&self, _: bool) -> io::Result<()> {
        match self.0 {}
    }

    pub fn recv(&self, _: &mut [u8]) -> io::Result<usize> {
        match self.0 {}
    }

    pub fn peek(&self, _: &mut [u8]) -> io::Result<usize> {
        match self.0 {}
    }

    pub fn send(&self, _: &[u8]) -> io::Result<usize> {
        match self.0 {}
    }

    pub fn connect(&self, _: &SocketAddr) -> io::Result<()> {
        match self.0 {}
    }
}

impl fmt::Debug for UdpSocket {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {}
    }
}

pub struct LookupHost(Void);

impl Iterator for LookupHost {
    type Item = SocketAddr;
    fn next(&mut self) -> Option<SocketAddr> {
        match self.0 {}
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(0))
    }
}

pub fn lookup_host(_: &str) -> io::Result<LookupHost> {
    unsupported()
}

#[allow(bad_style)]
pub mod netc {
    pub const AF_INET: u8 = 0;
    pub const AF_INET6: u8 = 1;
    pub type sa_family_t = u8;

    #[derive(Copy, Clone)]
    pub struct in_addr {
        pub s_addr: u32,
    }

    #[derive(Copy, Clone)]
    pub struct sockaddr_in {
        pub sin_family: sa_family_t,
        pub sin_port: u16,
        pub sin_addr: in_addr,
    }

    #[derive(Copy, Clone)]
    pub struct in6_addr {
        pub s6_addr: [u8; 16],
    }

    #[derive(Copy, Clone)]
    pub struct sockaddr_in6 {
        pub sin6_family: sa_family_t,
        pub sin6_port: u16,
        pub sin6_addr: in6_addr,
        pub sin6_flowinfo: u32,
        pub sin6_scope_id: u32,
    }

    #[derive(Copy, Clone)]
    pub struct sockaddr {
    }

    pub type socklen_t = usize;
}
