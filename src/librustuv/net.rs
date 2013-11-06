// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cast;
use std::libc::{size_t, ssize_t, c_int, c_void, c_uint, c_char};
use std::ptr;
use std::rt::BlockedTask;
use std::rt::io::IoError;
use std::rt::io::net::ip::{Ipv4Addr, Ipv6Addr, SocketAddr, IpAddr};
use std::rt::local::Local;
use std::rt::rtio;
use std::rt::sched::{Scheduler, SchedHandle};
use std::rt::tube::Tube;
use std::str;
use std::vec;

use stream::StreamWatcher;
use super::{Loop, Request, UvError, Buf, status_to_io_result,
            uv_error_to_io_error, UvHandle, slice_to_uv_buf};
use uvio::HomingIO;
use uvll;

////////////////////////////////////////////////////////////////////////////////
/// Generic functions related to dealing with sockaddr things
////////////////////////////////////////////////////////////////////////////////

pub enum UvSocketAddr {
    UvIpv4SocketAddr(*uvll::sockaddr_in),
    UvIpv6SocketAddr(*uvll::sockaddr_in6),
}

pub fn sockaddr_to_UvSocketAddr(addr: *uvll::sockaddr) -> UvSocketAddr {
    unsafe {
        assert!((uvll::is_ip4_addr(addr) || uvll::is_ip6_addr(addr)));
        assert!(!(uvll::is_ip4_addr(addr) && uvll::is_ip6_addr(addr)));
        match addr {
            _ if uvll::is_ip4_addr(addr) =>
                UvIpv4SocketAddr(addr as *uvll::sockaddr_in),
            _ if uvll::is_ip6_addr(addr) =>
                UvIpv6SocketAddr(addr as *uvll::sockaddr_in6),
            _ => fail!(),
        }
    }
}

fn socket_addr_as_uv_socket_addr<T>(addr: SocketAddr, f: &fn(UvSocketAddr) -> T) -> T {
    let malloc = match addr.ip {
        Ipv4Addr(*) => uvll::malloc_ip4_addr,
        Ipv6Addr(*) => uvll::malloc_ip6_addr,
    };
    let wrap = match addr.ip {
        Ipv4Addr(*) => UvIpv4SocketAddr,
        Ipv6Addr(*) => UvIpv6SocketAddr,
    };
    let free = match addr.ip {
        Ipv4Addr(*) => uvll::free_ip4_addr,
        Ipv6Addr(*) => uvll::free_ip6_addr,
    };

    let addr = unsafe { malloc(addr.ip.to_str(), addr.port as int) };
    do (|| {
        f(wrap(addr))
    }).finally {
        unsafe { free(addr) };
    }
}

fn uv_socket_addr_as_socket_addr<T>(addr: UvSocketAddr, f: &fn(SocketAddr) -> T) -> T {
    let ip_size = match addr {
        UvIpv4SocketAddr(*) => 4/*groups of*/ * 3/*digits separated by*/ + 3/*periods*/,
        UvIpv6SocketAddr(*) => 8/*groups of*/ * 4/*hex digits separated by*/ + 7 /*colons*/,
    };
    let ip_name = {
        let buf = vec::from_elem(ip_size + 1 /*null terminated*/, 0u8);
        unsafe {
            let buf_ptr = vec::raw::to_ptr(buf);
            match addr {
                UvIpv4SocketAddr(addr) =>
                    uvll::uv_ip4_name(addr, buf_ptr as *c_char, ip_size as size_t),
                UvIpv6SocketAddr(addr) =>
                    uvll::uv_ip6_name(addr, buf_ptr as *c_char, ip_size as size_t),
            }
        };
        buf
    };
    let ip_port = unsafe {
        let port = match addr {
            UvIpv4SocketAddr(addr) => uvll::ip4_port(addr),
            UvIpv6SocketAddr(addr) => uvll::ip6_port(addr),
        };
        port as u16
    };
    let ip_str = str::from_utf8_slice(ip_name).trim_right_chars(&'\x00');
    let ip_addr = FromStr::from_str(ip_str).unwrap();

    // finally run the closure
    f(SocketAddr { ip: ip_addr, port: ip_port })
}

pub fn uv_socket_addr_to_socket_addr(addr: UvSocketAddr) -> SocketAddr {
    use std::util;
    uv_socket_addr_as_socket_addr(addr, util::id)
}

#[cfg(test)]
#[test]
fn test_ip4_conversion() {
    use std::rt;
    let ip4 = rt::test::next_test_ip4();
    assert_eq!(ip4, socket_addr_as_uv_socket_addr(ip4, uv_socket_addr_to_socket_addr));
}

#[cfg(test)]
#[test]
fn test_ip6_conversion() {
    use std::rt;
    let ip6 = rt::test::next_test_ip6();
    assert_eq!(ip6, socket_addr_as_uv_socket_addr(ip6, uv_socket_addr_to_socket_addr));
}

enum SocketNameKind {
    TcpPeer,
    Tcp,
    Udp
}

fn socket_name(sk: SocketNameKind, handle: *c_void) -> Result<SocketAddr, IoError> {
    let getsockname = match sk {
        TcpPeer => uvll::tcp_getpeername,
        Tcp     => uvll::tcp_getsockname,
        Udp     => uvll::udp_getsockname,
    };

    // Allocate a sockaddr_storage
    // since we don't know if it's ipv4 or ipv6
    let r_addr = unsafe { uvll::malloc_sockaddr_storage() };

    let r = unsafe {
        getsockname(handle, r_addr as *uvll::sockaddr_storage)
    };

    if r != 0 {
        return Err(uv_error_to_io_error(UvError(r)));
    }

    let addr = unsafe {
        if uvll::is_ip6_addr(r_addr as *uvll::sockaddr) {
            uv_socket_addr_to_socket_addr(UvIpv6SocketAddr(r_addr as *uvll::sockaddr_in6))
        } else {
            uv_socket_addr_to_socket_addr(UvIpv4SocketAddr(r_addr as *uvll::sockaddr_in))
        }
    };

    unsafe { uvll::free_sockaddr_storage(r_addr); }

    Ok(addr)

}

////////////////////////////////////////////////////////////////////////////////
/// TCP implementation
////////////////////////////////////////////////////////////////////////////////

pub struct TcpWatcher {
    handle: *uvll::uv_tcp_t,
    stream: StreamWatcher,
    home: SchedHandle,
}

pub struct TcpListener {
    home: SchedHandle,
    handle: *uvll::uv_pipe_t,
    priv closing_task: Option<BlockedTask>,
    priv outgoing: Tube<Result<~rtio::RtioTcpStream, IoError>>,
}

pub struct TcpAcceptor {
    listener: ~TcpListener,
    priv incoming: Tube<Result<~rtio::RtioTcpStream, IoError>>,
}

// TCP watchers (clients/streams)

impl TcpWatcher {
    pub fn new(loop_: &Loop) -> TcpWatcher {
        let handle = unsafe { uvll::malloc_handle(uvll::UV_TCP) };
        assert_eq!(unsafe {
            uvll::uv_tcp_init(loop_.handle, handle)
        }, 0);
        TcpWatcher {
            home: get_handle_to_current_scheduler!(),
            handle: handle,
            stream: StreamWatcher::new(handle),
        }
    }

    pub fn connect(loop_: &mut Loop, address: SocketAddr)
        -> Result<TcpWatcher, UvError>
    {
        struct Ctx { status: c_int, task: Option<BlockedTask> }

        let tcp = TcpWatcher::new(loop_);
        let ret = do socket_addr_as_uv_socket_addr(address) |addr| {
            let req = Request::new(uvll::UV_CONNECT);
            let result = match addr {
                UvIpv4SocketAddr(addr) => unsafe {
                    uvll::tcp_connect(req.handle, tcp.handle, addr,
                                      connect_cb)
                },
                UvIpv6SocketAddr(addr) => unsafe {
                    uvll::tcp_connect6(req.handle, tcp.handle, addr,
                                       connect_cb)
                },
            };
            match result {
                0 => {
                    let mut cx = Ctx { status: 0, task: None };
                    req.set_data(&cx);
                    req.defuse();
                    let scheduler: ~Scheduler = Local::take();
                    do scheduler.deschedule_running_task_and_then |_, task| {
                        cx.task = Some(task);
                    }
                    match cx.status {
                        0 => Ok(()),
                        n => Err(UvError(n)),
                    }
                }
                n => Err(UvError(n))
            }
        };

        return match ret {
            Ok(()) => Ok(tcp),
            Err(e) => Err(e),
        };

        extern fn connect_cb(req: *uvll::uv_connect_t, status: c_int) {
            let req = Request::wrap(req);
            if status == uvll::ECANCELED { return }
            let cx: &mut Ctx = unsafe { cast::transmute(req.get_data()) };
            cx.status = status;
            let scheduler: ~Scheduler = Local::take();
            scheduler.resume_blocked_task_immediately(cx.task.take_unwrap());
        }
    }
}

impl HomingIO for TcpWatcher {
    fn home<'r>(&'r mut self) -> &'r mut SchedHandle { &mut self.home }
}

impl rtio::RtioSocket for TcpWatcher {
    fn socket_name(&mut self) -> Result<SocketAddr, IoError> {
        let _m = self.fire_homing_missile();
        socket_name(Tcp, self.handle)
    }
}

impl rtio::RtioTcpStream for TcpWatcher {
    fn read(&mut self, buf: &mut [u8]) -> Result<uint, IoError> {
        let _m = self.fire_homing_missile();
        self.stream.read(buf).map_err(uv_error_to_io_error)
    }

    fn write(&mut self, buf: &[u8]) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        self.stream.write(buf).map_err(uv_error_to_io_error)
    }

    fn peer_name(&mut self) -> Result<SocketAddr, IoError> {
        let _m = self.fire_homing_missile();
        socket_name(TcpPeer, self.handle)
    }

    fn control_congestion(&mut self) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_tcp_nodelay(self.handle, 0 as c_int)
        })
    }

    fn nodelay(&mut self) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_tcp_nodelay(self.handle, 1 as c_int)
        })
    }

    fn keepalive(&mut self, delay_in_seconds: uint) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_tcp_keepalive(self.handle, 1 as c_int,
                                   delay_in_seconds as c_uint)
        })
    }

    fn letdie(&mut self) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_tcp_keepalive(self.handle, 0 as c_int, 0 as c_uint)
        })
    }
}

impl Drop for TcpWatcher {
    fn drop(&mut self) {
        let _m = self.fire_homing_missile();
        self.stream.close();
    }
}

// TCP listeners (unbound servers)

impl TcpListener {
    pub fn bind(loop_: &mut Loop, address: SocketAddr)
        -> Result<~TcpListener, UvError>
    {
        let handle = unsafe { uvll::malloc_handle(uvll::UV_TCP) };
        assert_eq!(unsafe {
            uvll::uv_tcp_init(loop_.handle, handle)
        }, 0);
        let l = ~TcpListener {
            home: get_handle_to_current_scheduler!(),
            handle: handle,
            closing_task: None,
            outgoing: Tube::new(),
        };
        let res = socket_addr_as_uv_socket_addr(address, |addr| unsafe {
            match addr {
                UvIpv4SocketAddr(addr) => uvll::tcp_bind(l.handle, addr),
                UvIpv6SocketAddr(addr) => uvll::tcp_bind6(l.handle, addr),
            }
        });
        match res {
            0 => Ok(l.install()),
            n => Err(UvError(n))
        }
    }
}

impl HomingIO for TcpListener {
    fn home<'r>(&'r mut self) -> &'r mut SchedHandle { &mut self.home }
}

impl UvHandle<uvll::uv_tcp_t> for TcpListener {
    fn uv_handle(&self) -> *uvll::uv_tcp_t { self.handle }
}

impl rtio::RtioSocket for TcpListener {
    fn socket_name(&mut self) -> Result<SocketAddr, IoError> {
        let _m = self.fire_homing_missile();
        socket_name(Tcp, self.handle)
    }
}

impl rtio::RtioTcpListener for TcpListener {
    fn listen(mut ~self) -> Result<~rtio::RtioTcpAcceptor, IoError> {
        // create the acceptor object from ourselves
        let incoming = self.outgoing.clone();
        let mut acceptor = ~TcpAcceptor {
            listener: self,
            incoming: incoming,
        };

        let _m = acceptor.fire_homing_missile();
        // XXX: the 128 backlog should be configurable
        match unsafe { uvll::uv_listen(acceptor.listener.handle, 128, listen_cb) } {
            0 => Ok(acceptor as ~rtio::RtioTcpAcceptor),
            n => Err(uv_error_to_io_error(UvError(n))),
        }
    }
}

extern fn listen_cb(server: *uvll::uv_stream_t, status: c_int) {
    let msg = match status {
        0 => {
            let loop_ = Loop::wrap(unsafe {
                uvll::get_loop_for_uv_handle(server)
            });
            let client = TcpWatcher::new(&loop_);
            assert_eq!(unsafe { uvll::uv_accept(server, client.handle) }, 0);
            Ok(~client as ~rtio::RtioTcpStream)
        }
        uvll::ECANCELED => return,
        n => Err(uv_error_to_io_error(UvError(n)))
    };

    let tcp: &mut TcpListener = unsafe { UvHandle::from_uv_handle(&server) };
    tcp.outgoing.send(msg);
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        let (_m, sched) = self.fire_homing_missile_sched();

        do sched.deschedule_running_task_and_then |_, task| {
            self.closing_task = Some(task);
            unsafe { uvll::uv_close(self.handle, listener_close_cb) }
        }
    }
}

extern fn listener_close_cb(handle: *uvll::uv_handle_t) {
    let tcp: &mut TcpListener = unsafe { UvHandle::from_uv_handle(&handle) };
    unsafe { uvll::free_handle(handle) }

    let sched: ~Scheduler = Local::take();
    sched.resume_blocked_task_immediately(tcp.closing_task.take_unwrap());
}

// TCP acceptors (bound servers)

impl HomingIO for TcpAcceptor {
    fn home<'r>(&'r mut self) -> &'r mut SchedHandle { self.listener.home() }
}

impl rtio::RtioSocket for TcpAcceptor {
    fn socket_name(&mut self) -> Result<SocketAddr, IoError> {
        let _m = self.fire_homing_missile();
        socket_name(Tcp, self.listener.handle)
    }
}

impl rtio::RtioTcpAcceptor for TcpAcceptor {
    fn accept(&mut self) -> Result<~rtio::RtioTcpStream, IoError> {
        let _m = self.fire_homing_missile();
        self.incoming.recv()
    }

    fn accept_simultaneously(&mut self) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_tcp_simultaneous_accepts(self.listener.handle, 1)
        })
    }

    fn dont_accept_simultaneously(&mut self) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_tcp_simultaneous_accepts(self.listener.handle, 0)
        })
    }
}

////////////////////////////////////////////////////////////////////////////////
/// UDP implementation
////////////////////////////////////////////////////////////////////////////////

pub struct UdpWatcher {
    handle: *uvll::uv_udp_t,
    home: SchedHandle,
}

impl UdpWatcher {
    pub fn bind(loop_: &Loop, address: SocketAddr)
        -> Result<UdpWatcher, UvError>
    {
        let udp = UdpWatcher {
            handle: unsafe { uvll::malloc_handle(uvll::UV_UDP) },
            home: get_handle_to_current_scheduler!(),
        };
        assert_eq!(unsafe {
            uvll::uv_udp_init(loop_.handle, udp.handle)
        }, 0);
        let result = socket_addr_as_uv_socket_addr(address, |addr| unsafe {
            match addr {
                UvIpv4SocketAddr(addr) => uvll::udp_bind(udp.handle, addr, 0u32),
                UvIpv6SocketAddr(addr) => uvll::udp_bind6(udp.handle, addr, 0u32),
            }
        });
        match result {
            0 => Ok(udp),
            n => Err(UvError(n)),
        }
    }
}

impl HomingIO for UdpWatcher {
    fn home<'r>(&'r mut self) -> &'r mut SchedHandle { &mut self.home }
}

impl rtio::RtioSocket for UdpWatcher {
    fn socket_name(&mut self) -> Result<SocketAddr, IoError> {
        let _m = self.fire_homing_missile();
        socket_name(Udp, self.handle)
    }
}

impl rtio::RtioUdpSocket for UdpWatcher {
    fn recvfrom(&mut self, buf: &mut [u8])
        -> Result<(uint, SocketAddr), IoError>
    {
        struct Ctx {
            task: Option<BlockedTask>,
            buf: Option<Buf>,
            result: Option<(ssize_t, SocketAddr)>,
        }
        let _m = self.fire_homing_missile();

        return match unsafe {
            uvll::uv_udp_recv_start(self.handle, alloc_cb, recv_cb)
        } {
            0 => {
                let mut cx = Ctx {
                    task: None,
                    buf: Some(slice_to_uv_buf(buf)),
                    result: None,
                };
                unsafe { uvll::set_data_for_uv_handle(self.handle, &cx) }
                let scheduler: ~Scheduler = Local::take();
                do scheduler.deschedule_running_task_and_then |_, task| {
                    cx.task = Some(task);
                }
                match cx.result.take_unwrap() {
                    (n, _) if n < 0 =>
                        Err(uv_error_to_io_error(UvError(n as c_int))),
                    (n, addr) => Ok((n as uint, addr))
                }
            }
            n => Err(uv_error_to_io_error(UvError(n)))
        };

        extern fn alloc_cb(handle: *uvll::uv_udp_t,
                           _suggested_size: size_t) -> Buf {
            let cx: &mut Ctx = unsafe {
                cast::transmute(uvll::get_data_for_uv_handle(handle))
            };
            cx.buf.take().expect("alloc_cb called more than once")
        }

        extern fn recv_cb(handle: *uvll::uv_udp_t, nread: ssize_t, _buf: Buf,
                          addr: *uvll::sockaddr, _flags: c_uint) {

            // When there's no data to read the recv callback can be a no-op.
            // This can happen if read returns EAGAIN/EWOULDBLOCK. By ignoring
            // this we just drop back to kqueue and wait for the next callback.
            if nread == 0 { return }
            if nread == uvll::ECANCELED as ssize_t { return }

            unsafe {
                assert_eq!(uvll::uv_udp_recv_stop(handle), 0)
            }

            let cx: &mut Ctx = unsafe {
                cast::transmute(uvll::get_data_for_uv_handle(handle))
            };
            let addr = sockaddr_to_UvSocketAddr(addr);
            let addr = uv_socket_addr_to_socket_addr(addr);
            cx.result = Some((nread, addr));

            let sched: ~Scheduler = Local::take();
            sched.resume_blocked_task_immediately(cx.task.take_unwrap());
        }
    }

    fn sendto(&mut self, buf: &[u8], dst: SocketAddr) -> Result<(), IoError> {
        struct Ctx { task: Option<BlockedTask>, result: c_int }

        let _m = self.fire_homing_missile();

        let req = Request::new(uvll::UV_UDP_SEND);
        let buf = slice_to_uv_buf(buf);
        let result = socket_addr_as_uv_socket_addr(dst, |dst| unsafe {
            match dst {
                UvIpv4SocketAddr(dst) =>
                    uvll::udp_send(req.handle, self.handle, [buf], dst, send_cb),
                UvIpv6SocketAddr(dst) =>
                    uvll::udp_send6(req.handle, self.handle, [buf], dst, send_cb),
            }
        });

        return match result {
            0 => {
                let mut cx = Ctx { task: None, result: 0 };
                req.set_data(&cx);
                req.defuse();

                let sched: ~Scheduler = Local::take();
                do sched.deschedule_running_task_and_then |_, task| {
                    cx.task = Some(task);
                }

                match cx.result {
                    0 => Ok(()),
                    n => Err(uv_error_to_io_error(UvError(n)))
                }
            }
            n => Err(uv_error_to_io_error(UvError(n)))
        };

        extern fn send_cb(req: *uvll::uv_udp_send_t, status: c_int) {
            let req = Request::wrap(req);
            let cx: &mut Ctx = unsafe { cast::transmute(req.get_data()) };
            cx.result = status;

            let sched: ~Scheduler = Local::take();
            sched.resume_blocked_task_immediately(cx.task.take_unwrap());
        }
    }

    fn join_multicast(&mut self, multi: IpAddr) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            do multi.to_str().with_c_str |m_addr| {
                uvll::uv_udp_set_membership(self.handle,
                                            m_addr, ptr::null(),
                                            uvll::UV_JOIN_GROUP)
            }
        })
    }

    fn leave_multicast(&mut self, multi: IpAddr) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            do multi.to_str().with_c_str |m_addr| {
                uvll::uv_udp_set_membership(self.handle,
                                            m_addr, ptr::null(),
                                            uvll::UV_LEAVE_GROUP)
            }
        })
    }

    fn loop_multicast_locally(&mut self) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_udp_set_multicast_loop(self.handle,
                                            1 as c_int)
        })
    }

    fn dont_loop_multicast_locally(&mut self) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_udp_set_multicast_loop(self.handle,
                                            0 as c_int)
        })
    }

    fn multicast_time_to_live(&mut self, ttl: int) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_udp_set_multicast_ttl(self.handle,
                                           ttl as c_int)
        })
    }

    fn time_to_live(&mut self, ttl: int) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_udp_set_ttl(self.handle, ttl as c_int)
        })
    }

    fn hear_broadcasts(&mut self) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_udp_set_broadcast(self.handle,
                                       1 as c_int)
        })
    }

    fn ignore_broadcasts(&mut self) -> Result<(), IoError> {
        let _m = self.fire_homing_missile();
        status_to_io_result(unsafe {
            uvll::uv_udp_set_broadcast(self.handle,
                                       0 as c_int)
        })
    }
}

impl Drop for UdpWatcher {
    fn drop(&mut self) {
        // Send ourselves home to close this handle (blocking while doing so).
        let (_m, sched) = self.fire_homing_missile_sched();
        let mut slot = None;
        unsafe {
            uvll::set_data_for_uv_handle(self.handle, &slot);
            uvll::uv_close(self.handle, close_cb);
        }
        do sched.deschedule_running_task_and_then |_, task| {
            slot = Some(task);
        }

        extern fn close_cb(handle: *uvll::uv_handle_t) {
            let slot: &mut Option<BlockedTask> = unsafe {
                cast::transmute(uvll::get_data_for_uv_handle(handle))
            };
            unsafe { uvll::free_handle(handle) }
            let sched: ~Scheduler = Local::take();
            sched.resume_blocked_task_immediately(slot.take_unwrap());
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
/// UV request support
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test {
    use std::cell::Cell;
    use std::comm::oneshot;
    use std::rt::test::*;
    use std::rt::rtio::{RtioTcpStream, RtioTcpListener, RtioTcpAcceptor,
                        RtioUdpSocket};
    use std::task;

    use super::*;
    use super::super::{Loop, run_uv_loop};

    #[test]
    fn connect_close_ip4() {
        do run_uv_loop |l| {
            match TcpWatcher::connect(l, next_test_ip4()) {
                Ok(*) => fail!(),
                Err(e) => assert_eq!(e.name(), ~"ECONNREFUSED"),
            }
        }
    }

    #[test]
    fn connect_close_ip6() {
        do run_uv_loop |l| {
            match TcpWatcher::connect(l, next_test_ip6()) {
                Ok(*) => fail!(),
                Err(e) => assert_eq!(e.name(), ~"ECONNREFUSED"),
            }
        }
    }

    #[test]
    fn udp_bind_close_ip4() {
        do run_uv_loop |l| {
            match UdpWatcher::bind(l, next_test_ip4()) {
                Ok(*) => {}
                Err(*) => fail!()
            }
        }
    }

    #[test]
    fn udp_bind_close_ip6() {
        do run_uv_loop |l| {
            match UdpWatcher::bind(l, next_test_ip6()) {
                Ok(*) => {}
                Err(*) => fail!()
            }
        }
    }

    #[test]
    fn listen_ip4() {
        do run_uv_loop |l| {
            let (port, chan) = oneshot();
            let chan = Cell::new(chan);
            let addr = next_test_ip4();

            let handle = l.handle;
            do spawn {
                let w = match TcpListener::bind(&mut Loop::wrap(handle), addr) {
                    Ok(w) => w, Err(e) => fail!("{:?}", e)
                };
                let mut w = match w.listen() {
                    Ok(w) => w, Err(e) => fail!("{:?}", e),
                };
                chan.take().send(());
                match w.accept() {
                    Ok(mut stream) => {
                        let mut buf = [0u8, ..10];
                        match stream.read(buf) {
                            Ok(10) => {} e => fail!("{:?}", e),
                        }
                        for i in range(0, 10u8) {
                            assert_eq!(buf[i], i + 1);
                        }
                    }
                    Err(e) => fail!("{:?}", e)
                }
            }

            port.recv();
            let mut w = match TcpWatcher::connect(&mut Loop::wrap(handle), addr) {
                Ok(w) => w, Err(e) => fail!("{:?}", e)
            };
            match w.write([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]) {
                Ok(()) => {}, Err(e) => fail!("{:?}", e)
            }
        }
    }

    #[test]
    fn listen_ip6() {
        do run_uv_loop |l| {
            let (port, chan) = oneshot();
            let chan = Cell::new(chan);
            let addr = next_test_ip6();

            let handle = l.handle;
            do spawn {
                let w = match TcpListener::bind(&mut Loop::wrap(handle), addr) {
                    Ok(w) => w, Err(e) => fail!("{:?}", e)
                };
                let mut w = match w.listen() {
                    Ok(w) => w, Err(e) => fail!("{:?}", e),
                };
                chan.take().send(());
                match w.accept() {
                    Ok(mut stream) => {
                        let mut buf = [0u8, ..10];
                        match stream.read(buf) {
                            Ok(10) => {} e => fail!("{:?}", e),
                        }
                        for i in range(0, 10u8) {
                            assert_eq!(buf[i], i + 1);
                        }
                    }
                    Err(e) => fail!("{:?}", e)
                }
            }

            port.recv();
            let mut w = match TcpWatcher::connect(&mut Loop::wrap(handle), addr) {
                Ok(w) => w, Err(e) => fail!("{:?}", e)
            };
            match w.write([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]) {
                Ok(()) => {}, Err(e) => fail!("{:?}", e)
            }
        }
    }

    #[test]
    fn udp_recv_ip4() {
        do run_uv_loop |l| {
            let (port, chan) = oneshot();
            let chan = Cell::new(chan);
            let client = next_test_ip4();
            let server = next_test_ip4();

            let handle = l.handle;
            do spawn {
                match UdpWatcher::bind(&mut Loop::wrap(handle), server) {
                    Ok(mut w) => {
                        chan.take().send(());
                        let mut buf = [0u8, ..10];
                        match w.recvfrom(buf) {
                            Ok((10, addr)) => assert_eq!(addr, client),
                            e => fail!("{:?}", e),
                        }
                        for i in range(0, 10u8) {
                            assert_eq!(buf[i], i + 1);
                        }
                    }
                    Err(e) => fail!("{:?}", e)
                }
            }

            port.recv();
            let mut w = match UdpWatcher::bind(&mut Loop::wrap(handle), client) {
                Ok(w) => w, Err(e) => fail!("{:?}", e)
            };
            match w.sendto([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], server) {
                Ok(()) => {}, Err(e) => fail!("{:?}", e)
            }
        }
    }

    #[test]
    fn udp_recv_ip6() {
        do run_uv_loop |l| {
            let (port, chan) = oneshot();
            let chan = Cell::new(chan);
            let client = next_test_ip6();
            let server = next_test_ip6();

            let handle = l.handle;
            do spawn {
                match UdpWatcher::bind(&mut Loop::wrap(handle), server) {
                    Ok(mut w) => {
                        chan.take().send(());
                        let mut buf = [0u8, ..10];
                        match w.recvfrom(buf) {
                            Ok((10, addr)) => assert_eq!(addr, client),
                            e => fail!("{:?}", e),
                        }
                        for i in range(0, 10u8) {
                            assert_eq!(buf[i], i + 1);
                        }
                    }
                    Err(e) => fail!("{:?}", e)
                }
            }

            port.recv();
            let mut w = match UdpWatcher::bind(&mut Loop::wrap(handle), client) {
                Ok(w) => w, Err(e) => fail!("{:?}", e)
            };
            match w.sendto([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], server) {
                Ok(()) => {}, Err(e) => fail!("{:?}", e)
            }
        }
    }

    #[test]
    fn test_read_read_read() {
        do run_uv_loop |l| {
            let addr = next_test_ip4();
            static MAX: uint = 500000;
            let (port, chan) = oneshot();
            let port = Cell::new(port);
            let chan = Cell::new(chan);

            let handle = l.handle;
            do spawntask {
                let l = &mut Loop::wrap(handle);
                let listener = TcpListener::bind(l, addr).unwrap();
                let mut acceptor = listener.listen().unwrap();
                chan.take().send(());
                let mut stream = acceptor.accept().unwrap();
                let buf = [1, .. 2048];
                let mut total_bytes_written = 0;
                while total_bytes_written < MAX {
                    stream.write(buf);
                    total_bytes_written += buf.len();
                }
            }

            do spawntask {
                let l = &mut Loop::wrap(handle);
                port.take().recv();
                let mut stream = TcpWatcher::connect(l, addr).unwrap();
                let mut buf = [0, .. 2048];
                let mut total_bytes_read = 0;
                while total_bytes_read < MAX {
                    let nread = stream.read(buf).unwrap();
                    uvdebug!("read {} bytes", nread);
                    total_bytes_read += nread;
                    for i in range(0u, nread) {
                        assert_eq!(buf[i], 1);
                    }
                }
                uvdebug!("read {} bytes total", total_bytes_read);
            }
        }
    }

    #[test]
    #[ignore(cfg(windows))] // FIXME(#10102) the server never sees the second send
    fn test_udp_twice() {
        do run_uv_loop |l| {
            let server_addr = next_test_ip4();
            let client_addr = next_test_ip4();
            let (port, chan) = oneshot();
            let port = Cell::new(port);
            let chan = Cell::new(chan);

            let handle = l.handle;
            do spawntask {
                let l = &mut Loop::wrap(handle);
                let mut client = UdpWatcher::bind(l, client_addr).unwrap();
                port.take().recv();
                assert!(client.sendto([1], server_addr).is_ok());
                assert!(client.sendto([2], server_addr).is_ok());
            }

            do spawntask {
                let l = &mut Loop::wrap(handle);
                let mut server = UdpWatcher::bind(l, server_addr).unwrap();
                chan.take().send(());
                let mut buf1 = [0];
                let mut buf2 = [0];
                let (nread1, src1) = server.recvfrom(buf1).unwrap();
                let (nread2, src2) = server.recvfrom(buf2).unwrap();
                assert_eq!(nread1, 1);
                assert_eq!(nread2, 1);
                assert_eq!(src1, client_addr);
                assert_eq!(src2, client_addr);
                assert_eq!(buf1[0], 1);
                assert_eq!(buf2[0], 2);
            }
        }
    }

    #[test]
    fn test_udp_many_read() {
        do run_uv_loop |l| {
            let server_out_addr = next_test_ip4();
            let server_in_addr = next_test_ip4();
            let client_out_addr = next_test_ip4();
            let client_in_addr = next_test_ip4();
            static MAX: uint = 500_000;

            let (p1, c1) = oneshot();
            let (p2, c2) = oneshot();

            let first = Cell::new((p1, c2));
            let second = Cell::new((p2, c1));

            let handle = l.handle;
            do spawntask {
                let l = &mut Loop::wrap(handle);
                let mut server_out = UdpWatcher::bind(l, server_out_addr).unwrap();
                let mut server_in = UdpWatcher::bind(l, server_in_addr).unwrap();
                let (port, chan) = first.take();
                chan.send(());
                port.recv();
                let msg = [1, .. 2048];
                let mut total_bytes_sent = 0;
                let mut buf = [1];
                while buf[0] == 1 {
                    // send more data
                    assert!(server_out.sendto(msg, client_in_addr).is_ok());
                    total_bytes_sent += msg.len();
                    // check if the client has received enough
                    let res = server_in.recvfrom(buf);
                    assert!(res.is_ok());
                    let (nread, src) = res.unwrap();
                    assert_eq!(nread, 1);
                    assert_eq!(src, client_out_addr);
                }
                assert!(total_bytes_sent >= MAX);
            }

            do spawntask {
                let l = &mut Loop::wrap(handle);
                let mut client_out = UdpWatcher::bind(l, client_out_addr).unwrap();
                let mut client_in = UdpWatcher::bind(l, client_in_addr).unwrap();
                let (port, chan) = second.take();
                port.recv();
                chan.send(());
                let mut total_bytes_recv = 0;
                let mut buf = [0, .. 2048];
                while total_bytes_recv < MAX {
                    // ask for more
                    assert!(client_out.sendto([1], server_in_addr).is_ok());
                    // wait for data
                    let res = client_in.recvfrom(buf);
                    assert!(res.is_ok());
                    let (nread, src) = res.unwrap();
                    assert_eq!(src, server_out_addr);
                    total_bytes_recv += nread;
                    for i in range(0u, nread) {
                        assert_eq!(buf[i], 1);
                    }
                }
                // tell the server we're done
                assert!(client_out.sendto([0], server_in_addr).is_ok());
            }
        }
    }

    #[test]
    fn test_read_and_block() {
        do run_uv_loop |l| {
            let addr = next_test_ip4();
            let (port, chan) = oneshot();
            let port = Cell::new(port);
            let chan = Cell::new(chan);

            let handle = l.handle;
            do spawntask {
                let l = &mut Loop::wrap(handle);
                let listener = TcpListener::bind(l, addr).unwrap();
                let mut acceptor = listener.listen().unwrap();
                let (port2, chan2) = stream();
                chan.take().send(port2);
                let mut stream = acceptor.accept().unwrap();
                let mut buf = [0, .. 2048];

                let expected = 32;
                let mut current = 0;
                let mut reads = 0;

                while current < expected {
                    let nread = stream.read(buf).unwrap();
                    for i in range(0u, nread) {
                        let val = buf[i] as uint;
                        assert_eq!(val, current % 8);
                        current += 1;
                    }
                    reads += 1;

                    chan2.send(());
                }

                // Make sure we had multiple reads
                assert!(reads > 1);
            }

            do spawntask {
                let l = &mut Loop::wrap(handle);
                let port2 = port.take().recv();
                let mut stream = TcpWatcher::connect(l, addr).unwrap();
                stream.write([0, 1, 2, 3, 4, 5, 6, 7]);
                stream.write([0, 1, 2, 3, 4, 5, 6, 7]);
                port2.recv();
                stream.write([0, 1, 2, 3, 4, 5, 6, 7]);
                stream.write([0, 1, 2, 3, 4, 5, 6, 7]);
                port2.recv();
            }
        }
    }

    #[test]
    fn test_simple_tcp_server_and_client_on_diff_threads() {
        let addr = next_test_ip4();

        do task::spawn_sched(task::SingleThreaded) {
            do run_uv_loop |l| {
                let listener = TcpListener::bind(l, addr).unwrap();
                let mut acceptor = listener.listen().unwrap();
                let mut stream = acceptor.accept().unwrap();
                let mut buf = [0, .. 2048];
                let nread = stream.read(buf).unwrap();
                assert_eq!(nread, 8);
                for i in range(0u, nread) {
                    assert_eq!(buf[i], i as u8);
                }
            }
        }

        do task::spawn_sched(task::SingleThreaded) {
            do run_uv_loop |l| {
                let mut stream = TcpWatcher::connect(l, addr);
                while stream.is_err() {
                    stream = TcpWatcher::connect(l, addr);
                }
                stream.unwrap().write([0, 1, 2, 3, 4, 5, 6, 7]);
            }
        }
    }

    // On one thread, create a udp socket. Then send that socket to another
    // thread and destroy the socket on the remote thread. This should make sure
    // that homing kicks in for the socket to go back home to the original
    // thread, close itself, and then come back to the last thread.
    #[test]
    fn test_homing_closes_correctly() {
        let (port, chan) = oneshot();
        let port = Cell::new(port);
        let chan = Cell::new(chan);

        do task::spawn_sched(task::SingleThreaded) {
            let chan = Cell::new(chan.take());
            do run_uv_loop |l| {
                let listener = UdpWatcher::bind(l, next_test_ip4()).unwrap();
                chan.take().send(listener);
            }
        }

        do task::spawn_sched(task::SingleThreaded) {
            let port = Cell::new(port.take());
            do run_uv_loop |_l| {
                port.take().recv();
            }
        }
    }

    // This is a bit of a crufty old test, but it has its uses.
    #[test]
    fn test_simple_homed_udp_io_bind_then_move_task_then_home_and_close() {
        use std::cast;
        use std::rt::local::Local;
        use std::rt::rtio::{EventLoop, IoFactory};
        use std::rt::sched::Scheduler;
        use std::rt::sched::{Shutdown, TaskFromFriend};
        use std::rt::sleeper_list::SleeperList;
        use std::rt::task::Task;
        use std::rt::task::UnwindResult;
        use std::rt::thread::Thread;
        use std::rt::work_queue::WorkQueue;
        use std::unstable::run_in_bare_thread;
        use uvio::UvEventLoop;

        do run_in_bare_thread {
            let sleepers = SleeperList::new();
            let work_queue1 = WorkQueue::new();
            let work_queue2 = WorkQueue::new();
            let queues = ~[work_queue1.clone(), work_queue2.clone()];

            let loop1 = ~UvEventLoop::new() as ~EventLoop;
            let mut sched1 = ~Scheduler::new(loop1, work_queue1, queues.clone(),
                                             sleepers.clone());
            let loop2 = ~UvEventLoop::new() as ~EventLoop;
            let mut sched2 = ~Scheduler::new(loop2, work_queue2, queues.clone(),
                                             sleepers.clone());

            let handle1 = Cell::new(sched1.make_handle());
            let handle2 = Cell::new(sched2.make_handle());
            let tasksFriendHandle = Cell::new(sched2.make_handle());

            let on_exit: ~fn(UnwindResult) = |exit_status| {
                handle1.take().send(Shutdown);
                handle2.take().send(Shutdown);
                assert!(exit_status.is_success());
            };

            unsafe fn local_io() -> &'static mut IoFactory {
                do Local::borrow |sched: &mut Scheduler| {
                    let mut io = None;
                    sched.event_loop.io(|i| io = Some(i));
                    cast::transmute(io.unwrap())
                }
            }

            let test_function: ~fn() = || {
                let io = unsafe { local_io() };
                let addr = next_test_ip4();
                let maybe_socket = io.udp_bind(addr);
                // this socket is bound to this event loop
                assert!(maybe_socket.is_ok());

                // block self on sched1
                do task::unkillable { // FIXME(#8674)
                    let scheduler: ~Scheduler = Local::take();
                    do scheduler.deschedule_running_task_and_then |_, task| {
                        // unblock task
                        do task.wake().map |task| {
                            // send self to sched2
                            tasksFriendHandle.take().send(TaskFromFriend(task));
                        };
                        // sched1 should now sleep since it has nothing else to do
                    }
                }
                // sched2 will wake up and get the task as we do nothing else,
                // the function ends and the socket goes out of scope sched2
                // will start to run the destructor the destructor will first
                // block the task, set it's home as sched1, then enqueue it
                // sched2 will dequeue the task, see that it has a home, and
                // send it to sched1 sched1 will wake up, exec the close
                // function on the correct loop, and then we're done
            };

            let mut main_task = ~Task::new_root(&mut sched1.stack_pool, None,
                                                test_function);
            main_task.death.on_exit = Some(on_exit);
            let main_task = Cell::new(main_task);

            let null_task = Cell::new(~do Task::new_root(&mut sched2.stack_pool,
                                                         None) || {});

            let sched1 = Cell::new(sched1);
            let sched2 = Cell::new(sched2);

            let thread1 = do Thread::start {
                sched1.take().bootstrap(main_task.take());
            };
            let thread2 = do Thread::start {
                sched2.take().bootstrap(null_task.take());
            };

            thread1.join();
            thread2.join();
        }
    }

}
