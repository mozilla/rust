//! Networking primitives for TCP/UDP communication.
//!
//! This module provides networking functionality for the Transmission Control and User
//! Datagram Protocols, as well as types for IP and socket addresses.
//!
//! # Organization
//!
//! * [`TcpListener`] and [`TcpStream`] provide functionality for communication over TCP
//! * [`UdpSocket`] provides functionality for communication over UDP
//! * [`IpAddr`] represents IP addresses of either IPv4 or IPv6; [`Ipv4Addr`] and
//!   [`Ipv6Addr`] are respectively IPv4 and IPv6 addresses
//! * [`SocketAddr`] represents socket addresses of either IPv4 or IPv6; [`SocketAddrV4`]
//!   and [`SocketAddrV6`] are respectively IPv4 and IPv6 socket addresses
//! * [`ToSocketAddrs`] is a trait that used for generic address resolution when interacting
//!   with networking objects like [`TcpListener`], [`TcpStream`] or [`UdpSocket`]
//! * Other types are return or parameter types for various methods in this module
//!
//! # Stability guarantees for IETF-defined behavior
//!
//! [`IpAddr`], [`Ipv4Addr`] and [`Ipv6Addr`] offer helper methods that provide information about
//! an address, for instance whether it is globally routable (see [`IpAddr::is_global()`]), or if
//! it is a multicast address (see [`IpAddr::is_multicast()`]). These methods are compliant with
//! the [IETF RFCs]. As erratas and new RFCs are published, these methods behavior may be subject
//! to changes.
//!
//! For instance, the `240/4` IPv4 block is currently reserved for "future use". If it is made
//! globally routable by an RFC, then in a future release [`Ipv4Addr::is_reserved()`] will return
//! `false` for such addresses, while [`Ipv4Addr::is_global`] will return true.
//!
//! [`IpAddr`]: crate::net::IpAddr
//! [`Ipv4Addr`]: crate::net::Ipv4Addr
//! [`IpAddr::is_global()`]: crate::net::IpAddr::is_global()
//! [`IpAddr::is_multicast()`]: crate::net::IpAddr::is_multicast()
//! [`Ipv4Addr::is_reserved()`]: crate::net::Ipv4Addr::is_reserved()
//! [`Ipv4Addr::is_global()`]: crate::net::Ipv4Addr::is_global()
//! [`Ipv6Addr`]: crate::net::Ipv6Addr
//! [`SocketAddr`]: crate::net::SocketAddr
//! [`SocketAddrV4`]: crate::net::SocketAddrV4
//! [`SocketAddrV6`]: crate::net::SocketAddrV6
//! [`TcpListener`]: crate::net::TcpListener
//! [`TcpStream`]: crate::net::TcpStream
//! [`ToSocketAddrs`]: crate::net::ToSocketAddrs
//! [`UdpSocket`]: crate::net::UdpSocket
//! [IETF RFCs]: https://tools.ietf.org/rfc/index

#![stable(feature = "rust1", since = "1.0.0")]

use crate::io::{self, Error, ErrorKind};

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::addr::{SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::ip::{IpAddr, Ipv4Addr, Ipv6Addr, Ipv6MulticastScope};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::parser::AddrParseError;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::tcp::{Incoming, TcpListener, TcpStream};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::udp::UdpSocket;

mod addr;
mod ip;
mod parser;
mod tcp;
#[cfg(test)]
mod test;
mod udp;

/// Possible values which can be passed to the [`TcpStream::shutdown`] method.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub enum Shutdown {
    /// The reading portion of the [`TcpStream`] should be shut down.
    ///
    /// All currently blocked and future [reads] will return [`Ok`]`(0)`.
    ///
    /// [reads]: crate::io::Read
    #[stable(feature = "rust1", since = "1.0.0")]
    Read,
    /// The writing portion of the [`TcpStream`] should be shut down.
    ///
    /// All currently blocked and future [writes] will return an error.
    ///
    /// [writes]: crate::io::Write
    #[stable(feature = "rust1", since = "1.0.0")]
    Write,
    /// Both the reading and the writing portions of the [`TcpStream`] should be shut down.
    ///
    /// See [`Shutdown::Read`] and [`Shutdown::Write`] for more information.
    #[stable(feature = "rust1", since = "1.0.0")]
    Both,
}

#[inline]
const fn htons(i: u16) -> u16 {
    i.to_be()
}
#[inline]
const fn ntohs(i: u16) -> u16 {
    u16::from_be(i)
}

fn each_addr<A: ToSocketAddrs, F, T>(addr: A, mut f: F) -> io::Result<T>
where
    F: FnMut(io::Result<&SocketAddr>) -> io::Result<T>,
{
    let addrs = match addr.to_socket_addrs() {
        Ok(addrs) => addrs,
        Err(e) => return f(Err(e)),
    };
    let mut last_err = None;
    for addr in addrs {
        match f(Ok(&addr)) {
            Ok(l) => return Ok(l),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        Error::new(ErrorKind::InvalidInput, "could not resolve to any addresses")
    }))
}
