use crate::cmp::Ordering;
use crate::fmt;
use crate::hash;
use crate::io::Write;
use crate::sys::net::netc as c;
use crate::sys_common::{AsInner, FromInner};

/// An IP address, either IPv4 or IPv6.
///
/// This enum can contain either an [`Ipv4Addr`] or an [`Ipv6Addr`], see their
/// respective documentation for more details.
///
/// The size of an `IpAddr` instance may vary depending on the target operating
/// system.
///
/// [`Ipv4Addr`]: ../../std/net/struct.Ipv4Addr.html
/// [`Ipv6Addr`]: ../../std/net/struct.Ipv6Addr.html
///
/// # Examples
///
/// ```
/// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
///
/// let localhost_v4 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
/// let localhost_v6 = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
///
/// assert_eq!("127.0.0.1".parse(), Ok(localhost_v4));
/// assert_eq!("::1".parse(), Ok(localhost_v6));
///
/// assert_eq!(localhost_v4.is_ipv6(), false);
/// assert_eq!(localhost_v4.is_ipv4(), true);
/// ```
#[stable(feature = "ip_addr", since = "1.7.0")]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, PartialOrd, Ord)]
pub enum IpAddr {
    /// An IPv4 address.
    #[stable(feature = "ip_addr", since = "1.7.0")]
    V4(#[stable(feature = "ip_addr", since = "1.7.0")] Ipv4Addr),
    /// An IPv6 address.
    #[stable(feature = "ip_addr", since = "1.7.0")]
    V6(#[stable(feature = "ip_addr", since = "1.7.0")] Ipv6Addr),
}

/// An IPv4 address.
///
/// IPv4 addresses are defined as 32-bit integers in [IETF RFC 791].
/// They are usually represented as four octets.
///
/// See [`IpAddr`] for a type encompassing both IPv4 and IPv6 addresses.
///
/// The size of an `Ipv4Addr` struct may vary depending on the target operating
/// system.
///
/// [IETF RFC 791]: https://tools.ietf.org/html/rfc791
/// [`IpAddr`]: ../../std/net/enum.IpAddr.html
///
/// # Textual representation
///
/// `Ipv4Addr` provides a [`FromStr`] implementation. The four octets are in decimal
/// notation, divided by `.` (this is called "dot-decimal notation").
///
/// [`FromStr`]: ../../std/str/trait.FromStr.html
///
/// # Examples
///
/// ```
/// use std::net::Ipv4Addr;
///
/// let localhost = Ipv4Addr::new(127, 0, 0, 1);
/// assert_eq!("127.0.0.1".parse(), Ok(localhost));
/// assert_eq!(localhost.is_loopback(), true);
/// ```
#[derive(Copy)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Ipv4Addr {
    inner: c::in_addr,
}

/// An IPv6 address.
///
/// IPv6 addresses are defined as 128-bit integers in [IETF RFC 4291].
/// They are usually represented as eight 16-bit segments.
///
/// See [`IpAddr`] for a type encompassing both IPv4 and IPv6 addresses.
///
/// The size of an `Ipv6Addr` struct may vary depending on the target operating
/// system.
///
/// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
/// [`IpAddr`]: ../../std/net/enum.IpAddr.html
///
/// # Textual representation
///
/// `Ipv6Addr` provides a [`FromStr`] implementation. There are many ways to represent
/// an IPv6 address in text, but in general, each segments is written in hexadecimal
/// notation, and segments are separated by `:`. For more information, see
/// [IETF RFC 5952].
///
/// [`FromStr`]: ../../std/str/trait.FromStr.html
/// [IETF RFC 5952]: https://tools.ietf.org/html/rfc5952
///
/// # Examples
///
/// ```
/// use std::net::Ipv6Addr;
///
/// let localhost = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
/// assert_eq!("::1".parse(), Ok(localhost));
/// assert_eq!(localhost.is_loopback(), true);
/// ```
#[derive(Copy)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Ipv6Addr {
    inner: c::in6_addr,
}

/// Scope of an IPv6 address as defined in [section 2 of IETF RFC 7346]
///
/// # Stability guarantees
///
/// This enum may [may by subject to changes][changes] in the
/// future, as new RFCs are published.
///
/// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
/// [section 2 of IETF RFC 7346]: https://tools.ietf.org/html/rfc7346#section-2
#[derive(Copy, PartialEq, Eq, Clone, Hash, Debug)]
#[non_exhaustive]
#[stable(feature = "ip", since = "1.42.0")]
pub enum Ipv6MulticastScope {
    /// Interface-Local scope. See [RFC 4291] and [RFC 7346]
    ///
    /// [RFC 4291]: https://tools.ietf.org/html/rfc4291#section-2.7
    /// [RFC 7346]: https://tools.ietf.org/html/rfc7346#section-2
    #[stable(feature = "ip", since = "1.42.0")]
    InterfaceLocal,

    /// Link-Local scope. See [RFC 4291] and [RFC 7346]
    ///
    /// [RFC 4291]: https://tools.ietf.org/html/rfc4291#section-2.7
    /// [RFC 7346]: https://tools.ietf.org/html/rfc7346#section-2
    #[stable(feature = "ip", since = "1.42.0")]
    LinkLocal,

    /// Realm-Local scope. See [RFC 4291] and [RFC 7346]
    ///
    /// [RFC 4291]: https://tools.ietf.org/html/rfc4291#section-2.7
    /// [RFC 7346]: https://tools.ietf.org/html/rfc7346#section-2
    #[stable(feature = "ip", since = "1.42.0")]
    RealmLocal,

    /// Admin-Local scope. See [RFC 4291] and [RFC 7346]
    ///
    /// [RFC 4291]: https://tools.ietf.org/html/rfc4291#section-2.7
    /// [RFC 7346]: https://tools.ietf.org/html/rfc7346#section-2
    #[stable(feature = "ip", since = "1.42.0")]
    AdminLocal,

    /// Site-Local scope. See [RFC 4291] and [RFC 7346]
    ///
    /// [RFC 4291]: https://tools.ietf.org/html/rfc4291#section-2.7
    /// [RFC 7346]: https://tools.ietf.org/html/rfc7346#section-2
    #[stable(feature = "ip", since = "1.42.0")]
    SiteLocal,

    /// Organization-Local scope. See [RFC 4291] and [RFC 7346]
    ///
    /// [RFC 4291]: https://tools.ietf.org/html/rfc4291#section-2.7
    /// [RFC 7346]: https://tools.ietf.org/html/rfc7346#section-2
    #[stable(feature = "ip", since = "1.42.0")]
    OrganizationLocal,

    /// Global scope. See [RFC 4291] and [RFC 7346]
    ///
    /// [RFC 4291]: https://tools.ietf.org/html/rfc4291#section-2.7
    /// [RFC 7346]: https://tools.ietf.org/html/rfc7346#section-2
    #[stable(feature = "ip", since = "1.42.0")]
    Global,

    /// Reserved scope. See [RFC 4291] and [RFC 7346]
    ///
    /// [RFC 4291]: https://tools.ietf.org/html/rfc4291#section-2.7
    /// [RFC 7346]: https://tools.ietf.org/html/rfc7346#section-2
    #[stable(feature = "ip", since = "1.42.0")]
    Reserved,

    /// Un-assigned scope. See [RFC 4291] and [RFC 7346]
    ///
    /// [RFC 4291]: https://tools.ietf.org/html/rfc4291#section-2.7
    /// [RFC 7346]: https://tools.ietf.org/html/rfc7346#section-2
    #[stable(feature = "ip", since = "1.42.0")]
    Unassigned,
}

impl IpAddr {
    /// Returns [`true`] for the special 'unspecified' address.
    ///
    /// See the documentation for [`Ipv4Addr::is_unspecified`][IPv4] and
    /// [`Ipv6Addr::is_unspecified`][IPv6] for more details.
    ///
    /// [IPv4]: ../../std/net/struct.Ipv4Addr.html#method.is_unspecified
    /// [IPv6]: ../../std/net/struct.Ipv6Addr.html#method.is_unspecified
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)).is_unspecified(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)).is_unspecified(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip_shared", since = "1.12.0")]
    pub fn is_unspecified(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_unspecified(),
            IpAddr::V6(ip) => ip.is_unspecified(),
        }
    }

    /// Returns [`true`] if this is a loopback address.
    ///
    /// See the documentation for [`Ipv4Addr::is_loopback`][IPv4] and
    /// [`Ipv6Addr::is_loopback`][IPv6] for more details.
    ///
    /// [IPv4]: ../../std/net/struct.Ipv4Addr.html#method.is_loopback
    /// [IPv6]: ../../std/net/struct.Ipv6Addr.html#method.is_loopback
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)).is_loopback(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0x1)).is_loopback(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip_shared", since = "1.12.0")]
    pub fn is_loopback(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_loopback(),
            IpAddr::V6(ip) => ip.is_loopback(),
        }
    }

    /// Returns [`true`] if the address appears to be globally routable.
    ///
    /// See the documentation for [`Ipv4Addr::is_global`][IPv4] and
    /// [`Ipv6Addr::is_global`][IPv6] for more details.
    ///
    /// [IPv4]: ../../std/net/struct.Ipv4Addr.html#method.is_global
    /// [IPv6]: ../../std/net/struct.Ipv6Addr.html#method.is_global
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(80, 9, 12, 3)).is_global(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0, 0, 0x1c9, 0, 0, 0xafc8, 0, 0x1)).is_global(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_global(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_global(),
            IpAddr::V6(ip) => ip.is_global(),
        }
    }

    /// Returns [`true`] if this is a multicast address.
    ///
    /// See the documentation for [`Ipv4Addr::is_multicast`][IPv4] and
    /// [`Ipv6Addr::is_multicast`][IPv6] for more details.
    ///
    /// [IPv4]: ../../std/net/struct.Ipv4Addr.html#method.is_multicast
    /// [IPv6]: ../../std/net/struct.Ipv6Addr.html#method.is_multicast
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(224, 254, 0, 0)).is_multicast(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0)).is_multicast(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip_shared", since = "1.12.0")]
    pub fn is_multicast(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_multicast(),
            IpAddr::V6(ip) => ip.is_multicast(),
        }
    }

    /// Returns [`true`] if this address is in a range designated for documentation.
    ///
    /// See the documentation for [`Ipv4Addr::is_documentation`][IPv4] and
    /// [`Ipv6Addr::is_documentation`][IPv6] for more details.
    ///
    /// [IPv4]: ../../std/net/struct.Ipv4Addr.html#method.is_documentation
    /// [IPv6]: ../../std/net/struct.Ipv6Addr.html#method.is_documentation
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 6)).is_documentation(), true);
    /// assert_eq!(
    ///     IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0)).is_documentation(),
    ///     true
    /// );
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_documentation(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_documentation(),
            IpAddr::V6(ip) => ip.is_documentation(),
        }
    }

    /// Returns [`true`] if this address is an [IPv4 address], and [`false`] otherwise.
    ///
    /// [`true`]: ../../std/primitive.bool.html
    /// [`false`]: ../../std/primitive.bool.html
    /// [IPv4 address]: #variant.V4
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 6)).is_ipv4(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0)).is_ipv4(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ipaddr_checker", since = "1.16.0")]
    pub fn is_ipv4(&self) -> bool {
        matches!(self, IpAddr::V4(_))
    }

    /// Returns [`true`] if this address is an [IPv6 address], and [`false`] otherwise.
    ///
    /// [`true`]: ../../std/primitive.bool.html
    /// [`false`]: ../../std/primitive.bool.html
    /// [IPv6 address]: #variant.V6
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 6)).is_ipv6(), false);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0)).is_ipv6(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ipaddr_checker", since = "1.16.0")]
    pub fn is_ipv6(&self) -> bool {
        matches!(self, IpAddr::V6(_))
    }
}

impl Ipv4Addr {
    /// Creates a new IPv4 address from four eight-bit octets.
    ///
    /// The result will represent the IP address `a`.`b`.`c`.`d`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::new(127, 0, 0, 1);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_ipv4", since = "1.32.0")]
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
        // FIXME: should just be u32::from_be_bytes([a, b, c, d]),
        // once that method is no longer rustc_const_unstable
        Ipv4Addr {
            inner: c::in_addr {
                s_addr: u32::to_be(
                    ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32),
                ),
            },
        }
    }

    /// An IPv4 address with the address pointing to localhost: 127.0.0.1.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::LOCALHOST;
    /// assert_eq!(addr, Ipv4Addr::new(127, 0, 0, 1));
    /// ```
    #[stable(feature = "ip_constructors", since = "1.30.0")]
    pub const LOCALHOST: Self = Ipv4Addr::new(127, 0, 0, 1);

    /// An IPv4 address representing an unspecified address: 0.0.0.0
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::UNSPECIFIED;
    /// assert_eq!(addr, Ipv4Addr::new(0, 0, 0, 0));
    /// ```
    #[stable(feature = "ip_constructors", since = "1.30.0")]
    pub const UNSPECIFIED: Self = Ipv4Addr::new(0, 0, 0, 0);

    /// An IPv4 address representing the broadcast address: 255.255.255.255
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::BROADCAST;
    /// assert_eq!(addr, Ipv4Addr::new(255, 255, 255, 255));
    /// ```
    #[stable(feature = "ip_constructors", since = "1.30.0")]
    pub const BROADCAST: Self = Ipv4Addr::new(255, 255, 255, 255);

    /// Returns the four eight-bit integers that make up this address.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::new(127, 0, 0, 1);
    /// assert_eq!(addr.octets(), [127, 0, 0, 1]);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn octets(&self) -> [u8; 4] {
        // This returns the order we want because s_addr is stored in big-endian.
        self.inner.s_addr.to_ne_bytes()
    }

    /// Returns [`true`] for the special 'unspecified' address (0.0.0.0).
    ///
    /// This property is defined in _UNIX Network Programming, Second Edition_,
    /// W. Richard Stevens, p. 891; see also [ip7].
    ///
    /// [ip7]: http://man7.org/linux/man-pages/man7/ip.7.html
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(0, 0, 0, 0).is_unspecified(), true);
    /// assert_eq!(Ipv4Addr::new(45, 22, 13, 197).is_unspecified(), false);
    /// ```
    #[stable(feature = "ip_shared", since = "1.12.0")]
    #[rustc_const_stable(feature = "const_ipv4", since = "1.32.0")]
    pub const fn is_unspecified(&self) -> bool {
        self.inner.s_addr == 0
    }

    /// Returns [`true`] if this is a loopback address (127.0.0.0/8).
    ///
    /// This property is defined by [IETF RFC 1122].
    ///
    /// [IETF RFC 1122]: https://tools.ietf.org/html/rfc1122
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(127, 0, 0, 1).is_loopback(), true);
    /// assert_eq!(Ipv4Addr::new(45, 22, 13, 197).is_loopback(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(since = "1.7.0", feature = "ip_17")]
    pub fn is_loopback(&self) -> bool {
        self.octets()[0] == 127
    }

    /// Returns [`true`] if this is a private address.
    ///
    /// The private address ranges are defined in [IETF RFC 1918] and include:
    ///
    ///  - 10.0.0.0/8
    ///  - 172.16.0.0/12
    ///  - 192.168.0.0/16
    ///
    /// [IETF RFC 1918]: https://tools.ietf.org/html/rfc1918
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(10, 0, 0, 1).is_private(), true);
    /// assert_eq!(Ipv4Addr::new(10, 10, 10, 10).is_private(), true);
    /// assert_eq!(Ipv4Addr::new(172, 16, 10, 10).is_private(), true);
    /// assert_eq!(Ipv4Addr::new(172, 29, 45, 14).is_private(), true);
    /// assert_eq!(Ipv4Addr::new(172, 32, 0, 2).is_private(), false);
    /// assert_eq!(Ipv4Addr::new(192, 168, 0, 2).is_private(), true);
    /// assert_eq!(Ipv4Addr::new(192, 169, 0, 2).is_private(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(since = "1.7.0", feature = "ip_17")]
    pub fn is_private(&self) -> bool {
        match self.octets() {
            [10, ..] => true,
            [172, b, ..] if b >= 16 && b <= 31 => true,
            [192, 168, ..] => true,
            _ => false,
        }
    }

    /// Returns [`true`] if the address is link-local (169.254.0.0/16).
    ///
    /// This property is defined by [IETF RFC 3927].
    ///
    /// [IETF RFC 3927]: https://tools.ietf.org/html/rfc3927
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(169, 254, 0, 0).is_link_local(), true);
    /// assert_eq!(Ipv4Addr::new(169, 254, 10, 65).is_link_local(), true);
    /// assert_eq!(Ipv4Addr::new(16, 89, 10, 65).is_link_local(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(since = "1.7.0", feature = "ip_17")]
    pub fn is_link_local(&self) -> bool {
        match self.octets() {
            [169, 254, ..] => true,
            _ => false,
        }
    }

    /// Returns [`true`] if the address appears to be globally routable.
    /// See [iana-ipv4-special-registry][ipv4-sr].
    ///
    /// The following return false:
    ///
    /// - private addresses (see [`is_private()`](#method.is_private))
    /// - the loopback address (see [`is_loopback()`](#method.is_loopback))
    /// - the link-local address (see [`is_link_local()`](#method.is_link_local))
    /// - the broadcast address (see [`is_broadcast()`](#method.is_broadcast))
    /// - addresses used for documentation (see [`is_documentation()`](#method.is_documentation))
    /// - the unspecified address (see [`is_unspecified()`](#method.is_unspecified)), and the whole
    ///   0.0.0.0/8 block
    /// - addresses reserved for future protocols (see
    /// [`is_ietf_protocol_assignment()`](#method.is_ietf_protocol_assignment), except
    /// `192.0.0.9/32` and `192.0.0.10/32` which are globally routable
    /// - addresses reserved for future use (see [`is_reserved()`](#method.is_reserved)
    /// - addresses reserved for networking devices benchmarking (see
    /// [`is_benchmarking`](#method.is_benchmarking))
    ///
    /// [ipv4-sr]: https://www.iana.org/assignments/iana-ipv4-special-registry/iana-ipv4-special-registry.xhtml
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// // private addresses are not global
    /// assert_eq!(Ipv4Addr::new(10, 254, 0, 0).is_global(), false);
    /// assert_eq!(Ipv4Addr::new(192, 168, 10, 65).is_global(), false);
    /// assert_eq!(Ipv4Addr::new(172, 16, 10, 65).is_global(), false);
    ///
    /// // the 0.0.0.0/8 block is not global
    /// assert_eq!(Ipv4Addr::new(0, 1, 2, 3).is_global(), false);
    /// // in particular, the unspecified address is not global
    /// assert_eq!(Ipv4Addr::new(0, 0, 0, 0).is_global(), false);
    ///
    /// // the loopback address is not global
    /// assert_eq!(Ipv4Addr::new(127, 0, 0, 1).is_global(), false);
    ///
    /// // link local addresses are not global
    /// assert_eq!(Ipv4Addr::new(169, 254, 45, 1).is_global(), false);
    ///
    /// // the broadcast address is not global
    /// assert_eq!(Ipv4Addr::new(255, 255, 255, 255).is_global(), false);
    ///
    /// // the address space designated for documentation is not global
    /// assert_eq!(Ipv4Addr::new(192, 0, 2, 255).is_global(), false);
    /// assert_eq!(Ipv4Addr::new(198, 51, 100, 65).is_global(), false);
    /// assert_eq!(Ipv4Addr::new(203, 0, 113, 6).is_global(), false);
    ///
    /// // shared addresses are not global
    /// assert_eq!(Ipv4Addr::new(100, 100, 0, 0).is_global(), false);
    ///
    /// // addresses reserved for protocol assignment are not global
    /// assert_eq!(Ipv4Addr::new(192, 0, 0, 0).is_global(), false);
    /// assert_eq!(Ipv4Addr::new(192, 0, 0, 255).is_global(), false);
    ///
    /// // addresses reserved for future use are not global
    /// assert_eq!(Ipv4Addr::new(250, 10, 20, 30).is_global(), false);
    ///
    /// // addresses reserved for network devices benchmarking are not global
    /// assert_eq!(Ipv4Addr::new(198, 18, 0, 0).is_global(), false);
    ///
    /// // All the other addresses are global
    /// assert_eq!(Ipv4Addr::new(1, 1, 1, 1).is_global(), true);
    /// assert_eq!(Ipv4Addr::new(80, 9, 12, 3).is_global(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_global(&self) -> bool {
        // check if this address is 192.0.0.9 or 192.0.0.10. These addresses are the only two
        // globally routable addresses in the 192.0.0.0/24 range.
        if u32::from(*self) == 0xc0000009 || u32::from(*self) == 0xc000000a {
            return true;
        }
        !self.is_private()
            && !self.is_loopback()
            && !self.is_link_local()
            && !self.is_broadcast()
            && !self.is_documentation()
            && !self.is_shared()
            && !self.is_ietf_protocol_assignment()
            && !self.is_reserved()
            && !self.is_benchmarking()
            // Make sure the address is not in 0.0.0.0/8
            && self.octets()[0] != 0
    }

    /// Returns [`true`] if this address is part of the Shared Address Space defined in
    /// [IETF RFC 6598] (`100.64.0.0/10`).
    ///
    /// [IETF RFC 6598]: https://tools.ietf.org/html/rfc6598
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(100, 64, 0, 0).is_shared(), true);
    /// assert_eq!(Ipv4Addr::new(100, 127, 255, 255).is_shared(), true);
    /// assert_eq!(Ipv4Addr::new(100, 128, 0, 0).is_shared(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_shared(&self) -> bool {
        self.octets()[0] == 100 && (self.octets()[1] & 0b1100_0000 == 0b0100_0000)
    }

    /// Returns [`true`] if this address is part of `192.0.0.0/24`, which is reserved to
    /// IANA for IETF protocol assignments, as documented in [IETF RFC 6890].
    ///
    /// Note that parts of this block are in use:
    ///
    /// - `192.0.0.8/32` is the "IPv4 dummy address" (see [IETF RFC 7600])
    /// - `192.0.0.9/32` is the "Port Control Protocol Anycast" (see [IETF RFC 7723])
    /// - `192.0.0.10/32` is used for NAT traversal (see [IETF RFC 8155])
    ///
    /// [IETF RFC 6890]: https://tools.ietf.org/html/rfc6890
    /// [IETF RFC 7600]: https://tools.ietf.org/html/rfc7600
    /// [IETF RFC 7723]: https://tools.ietf.org/html/rfc7723
    /// [IETF RFC 8155]: https://tools.ietf.org/html/rfc8155
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(192, 0, 0, 0).is_ietf_protocol_assignment(), true);
    /// assert_eq!(Ipv4Addr::new(192, 0, 0, 8).is_ietf_protocol_assignment(), true);
    /// assert_eq!(Ipv4Addr::new(192, 0, 0, 9).is_ietf_protocol_assignment(), true);
    /// assert_eq!(Ipv4Addr::new(192, 0, 0, 255).is_ietf_protocol_assignment(), true);
    /// assert_eq!(Ipv4Addr::new(192, 0, 1, 0).is_ietf_protocol_assignment(), false);
    /// assert_eq!(Ipv4Addr::new(191, 255, 255, 255).is_ietf_protocol_assignment(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_ietf_protocol_assignment(&self) -> bool {
        self.octets()[0] == 192 && self.octets()[1] == 0 && self.octets()[2] == 0
    }

    /// Returns [`true`] if this address part of the `198.18.0.0/15` range, which is reserved for
    /// network devices benchmarking. This range is defined in [IETF RFC 2544] as `192.18.0.0`
    /// through `198.19.255.255` but [errata 423] corrects it to `198.18.0.0/15`.
    ///
    /// [IETF RFC 2544]: https://tools.ietf.org/html/rfc2544
    /// [errata 423]: https://www.rfc-editor.org/errata/eid423
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(198, 17, 255, 255).is_benchmarking(), false);
    /// assert_eq!(Ipv4Addr::new(198, 18, 0, 0).is_benchmarking(), true);
    /// assert_eq!(Ipv4Addr::new(198, 19, 255, 255).is_benchmarking(), true);
    /// assert_eq!(Ipv4Addr::new(198, 20, 0, 0).is_benchmarking(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_benchmarking(&self) -> bool {
        self.octets()[0] == 198 && (self.octets()[1] & 0xfe) == 18
    }

    /// Returns [`true`] if this address is reserved by IANA for future use. [IETF RFC 1112]
    /// defines the block of reserved addresses as `240.0.0.0/4`. This range normally includes the
    /// broadcast address `255.255.255.255`, but this implementation explicitely excludes it, since
    /// it is obviously not reserved for future use.
    ///
    /// [IETF RFC 1112]: https://tools.ietf.org/html/rfc1112
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Warning
    ///
    /// As IANA assigns new addresses, this method will be
    /// updated. This may result in non-reserved addresses being
    /// treated as reserved in code that relies on an outdated version
    /// of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(240, 0, 0, 0).is_reserved(), true);
    /// assert_eq!(Ipv4Addr::new(255, 255, 255, 254).is_reserved(), true);
    ///
    /// assert_eq!(Ipv4Addr::new(239, 255, 255, 255).is_reserved(), false);
    /// // The broadcast address is not considered as reserved for future use by this implementation
    /// assert_eq!(Ipv4Addr::new(255, 255, 255, 255).is_reserved(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_reserved(&self) -> bool {
        self.octets()[0] & 240 == 240 && !self.is_broadcast()
    }

    /// Returns [`true`] if this is a multicast address (224.0.0.0/4).
    ///
    /// Multicast addresses have a most significant octet between 224 and 239,
    /// and is defined by [IETF RFC 5771].
    ///
    /// [IETF RFC 5771]: https://tools.ietf.org/html/rfc5771
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(224, 254, 0, 0).is_multicast(), true);
    /// assert_eq!(Ipv4Addr::new(236, 168, 10, 65).is_multicast(), true);
    /// assert_eq!(Ipv4Addr::new(172, 16, 10, 65).is_multicast(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(since = "1.7.0", feature = "ip_17")]
    pub fn is_multicast(&self) -> bool {
        self.octets()[0] >= 224 && self.octets()[0] <= 239
    }

    /// Returns [`true`] if this is a broadcast address (255.255.255.255).
    ///
    /// A broadcast address has all octets set to 255 as defined in [IETF RFC 919].
    ///
    /// [IETF RFC 919]: https://tools.ietf.org/html/rfc919
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(255, 255, 255, 255).is_broadcast(), true);
    /// assert_eq!(Ipv4Addr::new(236, 168, 10, 65).is_broadcast(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(since = "1.7.0", feature = "ip_17")]
    pub fn is_broadcast(&self) -> bool {
        self == &Self::BROADCAST
    }

    /// Returns [`true`] if this address is in a range designated for documentation.
    ///
    /// This is defined in [IETF RFC 5737]:
    ///
    /// - 192.0.2.0/24 (TEST-NET-1)
    /// - 198.51.100.0/24 (TEST-NET-2)
    /// - 203.0.113.0/24 (TEST-NET-3)
    ///
    /// [IETF RFC 5737]: https://tools.ietf.org/html/rfc5737
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(192, 0, 2, 255).is_documentation(), true);
    /// assert_eq!(Ipv4Addr::new(198, 51, 100, 65).is_documentation(), true);
    /// assert_eq!(Ipv4Addr::new(203, 0, 113, 6).is_documentation(), true);
    /// assert_eq!(Ipv4Addr::new(193, 34, 17, 19).is_documentation(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(since = "1.7.0", feature = "ip_17")]
    pub fn is_documentation(&self) -> bool {
        match self.octets() {
            [192, 0, 2, _] => true,
            [198, 51, 100, _] => true,
            [203, 0, 113, _] => true,
            _ => false,
        }
    }

    /// Converts this address to an IPv4-compatible [IPv6 address].
    ///
    /// a.b.c.d becomes ::a.b.c.d
    ///
    /// [IPv6 address]: ../../std/net/struct.Ipv6Addr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(
    ///     Ipv4Addr::new(192, 0, 2, 255).to_ipv6_compatible(),
    ///     Ipv6Addr::new(0, 0, 0, 0, 0, 0, 49152, 767)
    /// );
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn to_ipv6_compatible(&self) -> Ipv6Addr {
        let octets = self.octets();
        Ipv6Addr::from([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, octets[0], octets[1], octets[2], octets[3],
        ])
    }

    /// Converts this address to an IPv4-mapped [IPv6 address].
    ///
    /// a.b.c.d becomes ::ffff:a.b.c.d
    ///
    /// [IPv6 address]: ../../std/net/struct.Ipv6Addr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(Ipv4Addr::new(192, 0, 2, 255).to_ipv6_mapped(),
    ///            Ipv6Addr::new(0, 0, 0, 0, 0, 65535, 49152, 767));
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn to_ipv6_mapped(&self) -> Ipv6Addr {
        let octets = self.octets();
        Ipv6Addr::from([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF, octets[0], octets[1], octets[2], octets[3],
        ])
    }
}

#[stable(feature = "ip_addr", since = "1.7.0")]
impl fmt::Display for IpAddr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpAddr::V4(ip) => ip.fmt(fmt),
            IpAddr::V6(ip) => ip.fmt(fmt),
        }
    }
}

#[stable(feature = "ip_from_ip", since = "1.16.0")]
impl From<Ipv4Addr> for IpAddr {
    fn from(ipv4: Ipv4Addr) -> IpAddr {
        IpAddr::V4(ipv4)
    }
}

#[stable(feature = "ip_from_ip", since = "1.16.0")]
impl From<Ipv6Addr> for IpAddr {
    fn from(ipv6: Ipv6Addr) -> IpAddr {
        IpAddr::V6(ipv6)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for Ipv4Addr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        const IPV4_BUF_LEN: usize = 15; // Long enough for the longest possible IPv4 address
        let mut buf = [0u8; IPV4_BUF_LEN];
        let mut buf_slice = &mut buf[..];
        let octets = self.octets();
        // Note: The call to write should never fail, hence the unwrap
        write!(buf_slice, "{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3]).unwrap();
        let len = IPV4_BUF_LEN - buf_slice.len();
        // This unsafe is OK because we know what is being written to the buffer
        let buf = unsafe { crate::str::from_utf8_unchecked(&buf[..len]) };
        fmt.pad(buf)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for Ipv4Addr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Clone for Ipv4Addr {
    fn clone(&self) -> Ipv4Addr {
        *self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialEq for Ipv4Addr {
    fn eq(&self, other: &Ipv4Addr) -> bool {
        self.inner.s_addr == other.inner.s_addr
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialEq<Ipv4Addr> for IpAddr {
    fn eq(&self, other: &Ipv4Addr) -> bool {
        match self {
            IpAddr::V4(v4) => v4 == other,
            IpAddr::V6(_) => false,
        }
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialEq<IpAddr> for Ipv4Addr {
    fn eq(&self, other: &IpAddr) -> bool {
        match other {
            IpAddr::V4(v4) => self == v4,
            IpAddr::V6(_) => false,
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Eq for Ipv4Addr {}

#[stable(feature = "rust1", since = "1.0.0")]
impl hash::Hash for Ipv4Addr {
    fn hash<H: hash::Hasher>(&self, s: &mut H) {
        // `inner` is #[repr(packed)], so we need to copy `s_addr`.
        { self.inner.s_addr }.hash(s)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd for Ipv4Addr {
    fn partial_cmp(&self, other: &Ipv4Addr) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialOrd<Ipv4Addr> for IpAddr {
    fn partial_cmp(&self, other: &Ipv4Addr) -> Option<Ordering> {
        match self {
            IpAddr::V4(v4) => v4.partial_cmp(other),
            IpAddr::V6(_) => Some(Ordering::Greater),
        }
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialOrd<IpAddr> for Ipv4Addr {
    fn partial_cmp(&self, other: &IpAddr) -> Option<Ordering> {
        match other {
            IpAddr::V4(v4) => self.partial_cmp(v4),
            IpAddr::V6(_) => Some(Ordering::Less),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for Ipv4Addr {
    fn cmp(&self, other: &Ipv4Addr) -> Ordering {
        u32::from_be(self.inner.s_addr).cmp(&u32::from_be(other.inner.s_addr))
    }
}

impl AsInner<c::in_addr> for Ipv4Addr {
    fn as_inner(&self) -> &c::in_addr {
        &self.inner
    }
}
impl FromInner<c::in_addr> for Ipv4Addr {
    fn from_inner(addr: c::in_addr) -> Ipv4Addr {
        Ipv4Addr { inner: addr }
    }
}

#[stable(feature = "ip_u32", since = "1.1.0")]
impl From<Ipv4Addr> for u32 {
    /// Converts an `Ipv4Addr` into a host byte order `u32`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::new(13, 12, 11, 10);
    /// assert_eq!(0x0d0c0b0au32, u32::from(addr));
    /// ```
    fn from(ip: Ipv4Addr) -> u32 {
        let ip = ip.octets();
        u32::from_be_bytes(ip)
    }
}

#[stable(feature = "ip_u32", since = "1.1.0")]
impl From<u32> for Ipv4Addr {
    /// Converts a host byte order `u32` into an `Ipv4Addr`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::from(0x0d0c0b0au32);
    /// assert_eq!(Ipv4Addr::new(13, 12, 11, 10), addr);
    /// ```
    fn from(ip: u32) -> Ipv4Addr {
        Ipv4Addr::from(ip.to_be_bytes())
    }
}

#[stable(feature = "from_slice_v4", since = "1.9.0")]
impl From<[u8; 4]> for Ipv4Addr {
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::from([13u8, 12u8, 11u8, 10u8]);
    /// assert_eq!(Ipv4Addr::new(13, 12, 11, 10), addr);
    /// ```
    fn from(octets: [u8; 4]) -> Ipv4Addr {
        Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3])
    }
}

#[stable(feature = "ip_from_slice", since = "1.17.0")]
impl From<[u8; 4]> for IpAddr {
    /// Creates an `IpAddr::V4` from a four element byte array.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr};
    ///
    /// let addr = IpAddr::from([13u8, 12u8, 11u8, 10u8]);
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(13, 12, 11, 10)), addr);
    /// ```
    fn from(octets: [u8; 4]) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(octets))
    }
}

impl Ipv6Addr {
    /// Creates a new IPv6 address from eight 16-bit segments.
    ///
    /// The result will represent the IP address `a:b:c:d:e:f:g:h`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_ipv6", since = "1.32.0")]
    pub const fn new(a: u16, b: u16, c: u16, d: u16, e: u16, f: u16, g: u16, h: u16) -> Ipv6Addr {
        Ipv6Addr {
            inner: c::in6_addr {
                s6_addr: [
                    (a >> 8) as u8,
                    a as u8,
                    (b >> 8) as u8,
                    b as u8,
                    (c >> 8) as u8,
                    c as u8,
                    (d >> 8) as u8,
                    d as u8,
                    (e >> 8) as u8,
                    e as u8,
                    (f >> 8) as u8,
                    f as u8,
                    (g >> 8) as u8,
                    g as u8,
                    (h >> 8) as u8,
                    h as u8,
                ],
            },
        }
    }

    /// An IPv6 address representing localhost: `::1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::LOCALHOST;
    /// assert_eq!(addr, Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    /// ```
    #[stable(feature = "ip_constructors", since = "1.30.0")]
    pub const LOCALHOST: Self = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);

    /// An IPv6 address representing the unspecified address: `::`
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::UNSPECIFIED;
    /// assert_eq!(addr, Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0));
    /// ```
    #[stable(feature = "ip_constructors", since = "1.30.0")]
    pub const UNSPECIFIED: Self = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0);

    /// Returns the eight 16-bit segments that make up this address.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).segments(),
    ///            [0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff]);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn segments(&self) -> [u16; 8] {
        let arr = &self.inner.s6_addr;
        [
            u16::from_be_bytes([arr[0], arr[1]]),
            u16::from_be_bytes([arr[2], arr[3]]),
            u16::from_be_bytes([arr[4], arr[5]]),
            u16::from_be_bytes([arr[6], arr[7]]),
            u16::from_be_bytes([arr[8], arr[9]]),
            u16::from_be_bytes([arr[10], arr[11]]),
            u16::from_be_bytes([arr[12], arr[13]]),
            u16::from_be_bytes([arr[14], arr[15]]),
        ]
    }

    /// Returns [`true`] for the special 'unspecified' address (::).
    ///
    /// This property is defined in [IETF RFC 4291].
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_unspecified(), false);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).is_unspecified(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(since = "1.7.0", feature = "ip_17")]
    pub fn is_unspecified(&self) -> bool {
        self.segments() == [0, 0, 0, 0, 0, 0, 0, 0]
    }

    /// Returns [`true`] if this is a loopback address (::1).
    ///
    /// This property is defined in [IETF RFC 4291].
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_loopback(), false);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0x1).is_loopback(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(since = "1.7.0", feature = "ip_17")]
    pub fn is_loopback(&self) -> bool {
        self.segments() == [0, 0, 0, 0, 0, 0, 0, 1]
    }

    /// Returns [`true`] if the address appears to be globally routable.
    ///
    /// The following return [`false`]:
    ///
    /// - the loopback address
    /// - link-local and unique local unicast addresses
    /// - interface-, link-, realm-, admin- and site-local multicast addresses
    ///
    /// [`true`]: ../../std/primitive.bool.html
    /// [`false`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_global(), true);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0x1).is_global(), false);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0x1c9, 0, 0, 0xafc8, 0, 0x1).is_global(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_global(&self) -> bool {
        match self.multicast_scope() {
            Some(Ipv6MulticastScope::Global) => true,
            None => self.is_unicast_global(),
            _ => false,
        }
    }

    /// Returns [`true`] if this is a unique local address (`fc00::/7`).
    ///
    /// This property is defined in [IETF RFC 4193].
    ///
    /// [IETF RFC 4193]: https://tools.ietf.org/html/rfc4193
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_unique_local(), false);
    /// assert_eq!(Ipv6Addr::new(0xfc02, 0, 0, 0, 0, 0, 0, 0).is_unique_local(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_unique_local(&self) -> bool {
        (self.segments()[0] & 0xfe00) == 0xfc00
    }

    /// Returns [`true`] if the address is a unicast link-local address (`fe80::/64`).
    ///
    /// A common mis-conception is to think that "unicast link-local addresses start with
    /// `fe80::`", but the [IETF RFC 4291] actually defines a stricter format for these addresses:
    ///
    /// ```no_rust
    /// |   10     |
    /// |  bits    |         54 bits         |          64 bits           |
    /// +----------+-------------------------+----------------------------+
    /// |1111111010|           0             |       interface ID         |
    /// +----------+-------------------------+----------------------------+
    /// ```
    ///
    /// This method validates the format defined in the RFC and won't recognize the following
    /// addresses such as `fe80:0:0:1::` or `fe81::` as unicast link-local addresses for example.
    /// If you need a less strict validation use [`is_unicast_link_local()`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let ip = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0);
    /// assert!(ip.is_unicast_link_local_strict());
    ///
    /// let ip = Ipv6Addr::new(0xfe80, 0, 0, 0, 0xffff, 0xffff, 0xffff, 0xffff);
    /// assert!(ip.is_unicast_link_local_strict());
    ///
    /// let ip = Ipv6Addr::new(0xfe80, 0, 0, 1, 0, 0, 0, 0);
    /// assert!(!ip.is_unicast_link_local_strict());
    /// assert!(ip.is_unicast_link_local());
    ///
    /// let ip = Ipv6Addr::new(0xfe81, 0, 0, 0, 0, 0, 0, 0);
    /// assert!(!ip.is_unicast_link_local_strict());
    /// assert!(ip.is_unicast_link_local());
    /// ```
    ///
    /// # See also
    ///
    /// - [IETF RFC 4291 section 2.5.6]
    /// - [RFC 4291 errata 4406]
    /// - [`is_unicast_link_local()`]
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    /// [IETF RFC 4291 section 2.5.6]: https://tools.ietf.org/html/rfc4291#section-2.5.6
    /// [`true`]: ../../std/primitive.bool.html
    /// [RFC 4291 errata 4406]: https://www.rfc-editor.org/errata/eid4406
    /// [`is_unicast_link_local()`]: ../../std/net/struct.Ipv6Addr.html#method.is_unicast_link_local
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_unicast_link_local_strict(&self) -> bool {
        (self.segments()[0] & 0xffff) == 0xfe80
            && (self.segments()[1] & 0xffff) == 0
            && (self.segments()[2] & 0xffff) == 0
            && (self.segments()[3] & 0xffff) == 0
    }

    /// Returns [`true`] if the address is a unicast link-local address (`fe80::/10`).
    ///
    /// This method returns [`true`] for addresses in the range reserved by [RFC 4291 section 2.4],
    /// i.e. addresses with the following format:
    ///
    /// ```no_rust
    /// |   10     |
    /// |  bits    |         54 bits         |          64 bits           |
    /// +----------+-------------------------+----------------------------+
    /// |1111111010|    arbitratry value     |       interface ID         |
    /// +----------+-------------------------+----------------------------+
    /// ```
    ///
    /// As a result, this method consider addresses such as `fe80:0:0:1::` or `fe81::` to be
    /// unicast link-local addresses, whereas [`is_unicast_link_local_strict()`] does not. If you
    /// need a strict validation fully compliant with the RFC, use
    /// [`is_unicast_link_local_strict()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let ip = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0);
    /// assert!(ip.is_unicast_link_local());
    ///
    /// let ip = Ipv6Addr::new(0xfe80, 0, 0, 0, 0xffff, 0xffff, 0xffff, 0xffff);
    /// assert!(ip.is_unicast_link_local());
    ///
    /// let ip = Ipv6Addr::new(0xfe80, 0, 0, 1, 0, 0, 0, 0);
    /// assert!(ip.is_unicast_link_local());
    /// assert!(!ip.is_unicast_link_local_strict());
    ///
    /// let ip = Ipv6Addr::new(0xfe81, 0, 0, 0, 0, 0, 0, 0);
    /// assert!(ip.is_unicast_link_local());
    /// assert!(!ip.is_unicast_link_local_strict());
    /// ```
    ///
    /// # See also
    ///
    /// - [IETF RFC 4291 section 2.4]
    /// - [RFC 4291 errata 4406]
    ///
    /// [IETF RFC 4291 section 2.4]: https://tools.ietf.org/html/rfc4291#section-2.4
    /// [`true`]: ../../std/primitive.bool.html
    /// [RFC 4291 errata 4406]: https://www.rfc-editor.org/errata/eid4406
    /// [`is_unicast_link_local_strict()`]: ../../std/net/struct.Ipv6Addr.html#method.is_unicast_link_local_strict
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_unicast_link_local(&self) -> bool {
        (self.segments()[0] & 0xffc0) == 0xfe80
    }

    /// Returns [`true`] if this is a deprecated unicast site-local address (fec0::/10). The
    /// unicast site-local address format is defined in [RFC 4291 section 2.5.7] as:
    ///
    /// ```no_rust
    /// |   10     |
    /// |  bits    |         54 bits         |         64 bits            |
    /// +----------+-------------------------+----------------------------+
    /// |1111111011|        subnet ID        |       interface ID         |
    /// +----------+-------------------------+----------------------------+
    /// ```
    ///
    /// [`true`]: ../../std/primitive.bool.html
    /// [RFC 4291 section 2.5.7]: https://tools.ietf.org/html/rfc4291#section-2.5.7
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(
    ///     Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_unicast_site_local(),
    ///     false
    /// );
    /// assert_eq!(Ipv6Addr::new(0xfec2, 0, 0, 0, 0, 0, 0, 0).is_unicast_site_local(), true);
    /// ```
    ///
    /// # Warning
    ///
    /// As per [RFC 3879], the whole `FEC0::/10` prefix is
    /// deprecated. New software must not support site-local
    /// addresses.
    ///
    /// [RFC 3879]: https://tools.ietf.org/html/rfc3879
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_unicast_site_local(&self) -> bool {
        (self.segments()[0] & 0xffc0) == 0xfec0
    }

    /// Returns [`true`] if this is an address reserved for documentation
    /// (2001:db8::/32).
    ///
    /// This property is defined in [IETF RFC 3849].
    ///
    /// [IETF RFC 3849]: https://tools.ietf.org/html/rfc3849
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_documentation(), false);
    /// assert_eq!(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0).is_documentation(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_documentation(&self) -> bool {
        (self.segments()[0] == 0x2001) && (self.segments()[1] == 0xdb8)
    }

    /// Returns [`true`] if the address is a globally routable unicast address.
    ///
    /// The following return false:
    ///
    /// - the loopback address
    /// - the link-local addresses
    /// - unique local addresses
    /// - the unspecified address
    /// - the address range reserved for documentation
    ///
    /// This method returns [`true`] for site-local addresses as per [RFC 4291 section 2.5.7]
    ///
    /// ```no_rust
    /// The special behavior of [the site-local unicast] prefix defined in [RFC3513] must no longer
    /// be supported in new implementations (i.e., new implementations must treat this prefix as
    /// Global Unicast).
    /// ```
    ///
    /// [`true`]: ../../std/primitive.bool.html
    /// [RFC 4291 section 2.5.7]: https://tools.ietf.org/html/rfc4291#section-2.5.7
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0).is_unicast_global(), false);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_unicast_global(), true);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn is_unicast_global(&self) -> bool {
        !self.is_multicast()
            && !self.is_loopback()
            && !self.is_unicast_link_local()
            && !self.is_unique_local()
            && !self.is_unspecified()
            && !self.is_documentation()
    }

    /// Return the address's multicast scope if the address is multicast.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{Ipv6Addr, Ipv6MulticastScope};
    ///
    /// assert_eq!(
    ///     Ipv6Addr::new(0xff0e, 0, 0, 0, 0, 0, 0, 0).multicast_scope(),
    ///     Some(Ipv6MulticastScope::Global)
    /// );
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).multicast_scope(), None);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "ip", since = "1.42.0")]
    pub fn multicast_scope(&self) -> Option<Ipv6MulticastScope> {
        use Ipv6MulticastScope::*;
        if self.is_multicast() {
            Some(match self.segments()[0] & 0x000f {
                0 | 15 => Reserved,
                1 => InterfaceLocal,
                2 => LinkLocal,
                3 => RealmLocal,
                4 => AdminLocal,
                5 => SiteLocal,
                8 => OrganizationLocal,
                14 => Global,
                _ => Unassigned,
            })
        } else {
            None
        }
    }

    /// Returns [`true`] if this is a multicast address (ff00::/8).
    ///
    /// This property is defined by [IETF RFC 4291].
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    /// [`true`]: ../../std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0).is_multicast(), true);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_multicast(), false);
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(since = "1.7.0", feature = "ip_17")]
    pub fn is_multicast(&self) -> bool {
        (self.segments()[0] & 0xff00) == 0xff00
    }

    /// Converts this address to an [IPv4 address]. Returns [`None`] if this address is
    /// neither IPv4-compatible or IPv4-mapped.
    ///
    /// ::a.b.c.d and ::ffff:a.b.c.d become a.b.c.d
    ///
    /// [IPv4 address]: ../../std/net/struct.Ipv4Addr.html
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0).to_ipv4(), None);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).to_ipv4(),
    ///            Some(Ipv4Addr::new(192, 10, 2, 255)));
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1).to_ipv4(),
    ///            Some(Ipv4Addr::new(0, 0, 0, 1)));
    /// ```
    ///
    /// # Stability guarantees
    ///
    /// This method's behavior [may by subject to changes][changes] in the
    /// future, as new RFCs are published.
    ///
    /// [changes]: ../net/index.html#stability-guarantees-for-ietf-defined-behavior
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn to_ipv4(&self) -> Option<Ipv4Addr> {
        match self.segments() {
            [0, 0, 0, 0, 0, f, g, h] if f == 0 || f == 0xffff => {
                Some(Ipv4Addr::new((g >> 8) as u8, g as u8, (h >> 8) as u8, h as u8))
            }
            _ => None,
        }
    }

    /// Returns the sixteen eight-bit integers the IPv6 address consists of.
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0).octets(),
    ///            [255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    /// ```
    #[stable(feature = "ipv6_to_octets", since = "1.12.0")]
    #[rustc_const_stable(feature = "const_ipv6", since = "1.32.0")]
    pub const fn octets(&self) -> [u8; 16] {
        self.inner.s6_addr
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for Ipv6Addr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Note: The calls to write should never fail, hence the unwraps in the function
        // Long enough for the longest possible IPv6: 39
        const IPV6_BUF_LEN: usize = 39;
        let mut buf = [0u8; IPV6_BUF_LEN];
        let mut buf_slice = &mut buf[..];

        match self.segments() {
            // We need special cases for :: and ::1, otherwise they're formatted
            // as ::0.0.0.[01]
            [0, 0, 0, 0, 0, 0, 0, 0] => write!(buf_slice, "::").unwrap(),
            [0, 0, 0, 0, 0, 0, 0, 1] => write!(buf_slice, "::1").unwrap(),
            // Ipv4 Compatible address
            [0, 0, 0, 0, 0, 0, g, h] => {
                write!(
                    buf_slice,
                    "::{}.{}.{}.{}",
                    (g >> 8) as u8,
                    g as u8,
                    (h >> 8) as u8,
                    h as u8
                )
                .unwrap();
            }
            // Ipv4-Mapped address
            [0, 0, 0, 0, 0, 0xffff, g, h] => {
                write!(
                    buf_slice,
                    "::ffff:{}.{}.{}.{}",
                    (g >> 8) as u8,
                    g as u8,
                    (h >> 8) as u8,
                    h as u8
                )
                .unwrap();
            }
            _ => {
                fn find_zero_slice(segments: &[u16; 8]) -> (usize, usize) {
                    let mut longest_span_len = 0;
                    let mut longest_span_at = 0;
                    let mut cur_span_len = 0;
                    let mut cur_span_at = 0;

                    for i in 0..8 {
                        if segments[i] == 0 {
                            if cur_span_len == 0 {
                                cur_span_at = i;
                            }

                            cur_span_len += 1;

                            if cur_span_len > longest_span_len {
                                longest_span_len = cur_span_len;
                                longest_span_at = cur_span_at;
                            }
                        } else {
                            cur_span_len = 0;
                            cur_span_at = 0;
                        }
                    }

                    (longest_span_at, longest_span_len)
                }

                let (zeros_at, zeros_len) = find_zero_slice(&self.segments());

                if zeros_len > 1 {
                    fn fmt_subslice(segments: &[u16], buf: &mut &mut [u8]) {
                        if !segments.is_empty() {
                            write!(*buf, "{:x}", segments[0]).unwrap();
                            for &seg in &segments[1..] {
                                write!(*buf, ":{:x}", seg).unwrap();
                            }
                        }
                    }

                    fmt_subslice(&self.segments()[..zeros_at], &mut buf_slice);
                    write!(buf_slice, "::").unwrap();
                    fmt_subslice(&self.segments()[zeros_at + zeros_len..], &mut buf_slice);
                } else {
                    let &[a, b, c, d, e, f, g, h] = &self.segments();
                    write!(
                        buf_slice,
                        "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
                        a, b, c, d, e, f, g, h
                    )
                    .unwrap();
                }
            }
        }
        let len = IPV6_BUF_LEN - buf_slice.len();
        // This is safe because we know exactly what can be in this buffer
        let buf = unsafe { crate::str::from_utf8_unchecked(&buf[..len]) };
        fmt.pad(buf)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for Ipv6Addr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Clone for Ipv6Addr {
    fn clone(&self) -> Ipv6Addr {
        *self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialEq for Ipv6Addr {
    fn eq(&self, other: &Ipv6Addr) -> bool {
        self.inner.s6_addr == other.inner.s6_addr
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialEq<IpAddr> for Ipv6Addr {
    fn eq(&self, other: &IpAddr) -> bool {
        match other {
            IpAddr::V4(_) => false,
            IpAddr::V6(v6) => self == v6,
        }
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialEq<Ipv6Addr> for IpAddr {
    fn eq(&self, other: &Ipv6Addr) -> bool {
        match self {
            IpAddr::V4(_) => false,
            IpAddr::V6(v6) => v6 == other,
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Eq for Ipv6Addr {}

#[stable(feature = "rust1", since = "1.0.0")]
impl hash::Hash for Ipv6Addr {
    fn hash<H: hash::Hasher>(&self, s: &mut H) {
        self.inner.s6_addr.hash(s)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd for Ipv6Addr {
    fn partial_cmp(&self, other: &Ipv6Addr) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialOrd<Ipv6Addr> for IpAddr {
    fn partial_cmp(&self, other: &Ipv6Addr) -> Option<Ordering> {
        match self {
            IpAddr::V4(_) => Some(Ordering::Less),
            IpAddr::V6(v6) => v6.partial_cmp(other),
        }
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialOrd<IpAddr> for Ipv6Addr {
    fn partial_cmp(&self, other: &IpAddr) -> Option<Ordering> {
        match other {
            IpAddr::V4(_) => Some(Ordering::Greater),
            IpAddr::V6(v6) => self.partial_cmp(v6),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for Ipv6Addr {
    fn cmp(&self, other: &Ipv6Addr) -> Ordering {
        self.segments().cmp(&other.segments())
    }
}

impl AsInner<c::in6_addr> for Ipv6Addr {
    fn as_inner(&self) -> &c::in6_addr {
        &self.inner
    }
}
impl FromInner<c::in6_addr> for Ipv6Addr {
    fn from_inner(addr: c::in6_addr) -> Ipv6Addr {
        Ipv6Addr { inner: addr }
    }
}

#[stable(feature = "i128", since = "1.26.0")]
impl From<Ipv6Addr> for u128 {
    /// Convert an `Ipv6Addr` into a host byte order `u128`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::new(
    ///     0x1020, 0x3040, 0x5060, 0x7080,
    ///     0x90A0, 0xB0C0, 0xD0E0, 0xF00D,
    /// );
    /// assert_eq!(0x102030405060708090A0B0C0D0E0F00D_u128, u128::from(addr));
    /// ```
    fn from(ip: Ipv6Addr) -> u128 {
        let ip = ip.octets();
        u128::from_be_bytes(ip)
    }
}
#[stable(feature = "i128", since = "1.26.0")]
impl From<u128> for Ipv6Addr {
    /// Convert a host byte order `u128` into an `Ipv6Addr`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::from(0x102030405060708090A0B0C0D0E0F00D_u128);
    /// assert_eq!(
    ///     Ipv6Addr::new(
    ///         0x1020, 0x3040, 0x5060, 0x7080,
    ///         0x90A0, 0xB0C0, 0xD0E0, 0xF00D,
    ///     ),
    ///     addr);
    /// ```
    fn from(ip: u128) -> Ipv6Addr {
        Ipv6Addr::from(ip.to_be_bytes())
    }
}

#[stable(feature = "ipv6_from_octets", since = "1.9.0")]
impl From<[u8; 16]> for Ipv6Addr {
    fn from(octets: [u8; 16]) -> Ipv6Addr {
        let inner = c::in6_addr { s6_addr: octets };
        Ipv6Addr::from_inner(inner)
    }
}

#[stable(feature = "ipv6_from_segments", since = "1.16.0")]
impl From<[u16; 8]> for Ipv6Addr {
    fn from(segments: [u16; 8]) -> Ipv6Addr {
        let [a, b, c, d, e, f, g, h] = segments;
        Ipv6Addr::new(a, b, c, d, e, f, g, h)
    }
}

#[stable(feature = "ip_from_slice", since = "1.17.0")]
impl From<[u8; 16]> for IpAddr {
    /// Creates an `IpAddr::V6` from a sixteen element byte array.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv6Addr};
    ///
    /// let addr = IpAddr::from([
    ///     25u8, 24u8, 23u8, 22u8, 21u8, 20u8, 19u8, 18u8,
    ///     17u8, 16u8, 15u8, 14u8, 13u8, 12u8, 11u8, 10u8,
    /// ]);
    /// assert_eq!(
    ///     IpAddr::V6(Ipv6Addr::new(
    ///         0x1918, 0x1716,
    ///         0x1514, 0x1312,
    ///         0x1110, 0x0f0e,
    ///         0x0d0c, 0x0b0a
    ///     )),
    ///     addr
    /// );
    /// ```
    fn from(octets: [u8; 16]) -> IpAddr {
        IpAddr::V6(Ipv6Addr::from(octets))
    }
}

#[stable(feature = "ip_from_slice", since = "1.17.0")]
impl From<[u16; 8]> for IpAddr {
    /// Creates an `IpAddr::V6` from an eight element 16-bit array.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv6Addr};
    ///
    /// let addr = IpAddr::from([
    ///     525u16, 524u16, 523u16, 522u16,
    ///     521u16, 520u16, 519u16, 518u16,
    /// ]);
    /// assert_eq!(
    ///     IpAddr::V6(Ipv6Addr::new(
    ///         0x20d, 0x20c,
    ///         0x20b, 0x20a,
    ///         0x209, 0x208,
    ///         0x207, 0x206
    ///     )),
    ///     addr
    /// );
    /// ```
    fn from(segments: [u16; 8]) -> IpAddr {
        IpAddr::V6(Ipv6Addr::from(segments))
    }
}
