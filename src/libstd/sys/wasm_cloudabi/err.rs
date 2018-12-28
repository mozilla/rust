// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use io::{Result, Error, ErrorKind};
use sys::wasm_cloudabi::cloudabi::errno;

pub fn cvt(no: errno) -> Result<()> {
    match no {
        errno::SUCCESS => Ok(()),
        errno::TOOBIG => Err(Error::new(ErrorKind::Other, "Argument list too long")),
        errno::ACCES => Err(Error::new(ErrorKind::PermissionDenied, "Permission denied")),
        errno::ADDRINUSE => Err(Error::new(ErrorKind::AddrInUse, "Address in use")),
        errno::ADDRNOTAVAIL => Err(Error::new(ErrorKind::AddrNotAvailable,
                                              "Address not available")),
        errno::AFNOSUPPORT => Err(Error::new(ErrorKind::Other, "Address family not supported")),
        errno::AGAIN => Err(Error::new(ErrorKind::WouldBlock,
                                       "Resource unavailable, or operation would block")),
        errno::ALREADY => Err(Error::new(ErrorKind::Other, "Connection already in progress")),
        errno::BADF => Err(Error::new(ErrorKind::Other, "Bad file descriptor")),
        errno::BADMSG => Err(Error::new(ErrorKind::Other, "Bad message")),
        errno::BUSY => Err(Error::new(ErrorKind::Other, "Device or resource busy")),
        errno::CANCELED => Err(Error::new(ErrorKind::Interrupted, "Operation canceled")),
        errno::CHILD => Err(Error::new(ErrorKind::Other, "No child processes")),
        errno::CONNABORTED => Err(Error::new(ErrorKind::ConnectionAborted, "Connection aborted")),
        errno::CONNREFUSED => Err(Error::new(ErrorKind::ConnectionRefused, "Connection refused")),
        errno::CONNRESET => Err(Error::new(ErrorKind::ConnectionReset, "Connection reset")),
        errno::DEADLK => Err(Error::new(ErrorKind::Other, "Resource deadlock would occur")),
        errno::DESTADDRREQ => Err(Error::new(ErrorKind::Other, "Destination address required")),
        errno::DOM => Err(Error::new(ErrorKind::Other,
                                     "Mathematics argument out of domain of function")),
        errno::DQUOT => Err(Error::new(ErrorKind::Other, "Reserved")),
        errno::EXIST => Err(Error::new(ErrorKind::AlreadyExists, "File exists")),
        errno::FAULT => Err(Error::new(ErrorKind::Other, "Bad address")),
        errno::FBIG => Err(Error::new(ErrorKind::Other, "File too large")),
        errno::HOSTUNREACH => Err(Error::new(ErrorKind::Other, "Host is unreachable")),
        errno::IDRM => Err(Error::new(ErrorKind::Other, "Identifier removed")),
        errno::ILSEQ => Err(Error::new(ErrorKind::Other, "Illegal byte sequence")),
        errno::INPROGRESS => Err(Error::new(ErrorKind::Other, "Operation in progress")),
        errno::INTR => Err(Error::new(ErrorKind::Other, "Interrupted function")),
        errno::INVAL => Err(Error::new(ErrorKind::InvalidInput, "Invalid argument")),
        errno::IO => Err(Error::new(ErrorKind::Other, "I/O error")),
        errno::ISCONN => Err(Error::new(ErrorKind::Other, "Socket is connected")),
        errno::ISDIR => Err(Error::new(ErrorKind::Other, "Is a directory")),
        errno::LOOP => Err(Error::new(ErrorKind::Other, "Too many levels of symbolic links")),
        errno::MFILE => Err(Error::new(ErrorKind::Other, "File descriptor value too large")),
        errno::MLINK => Err(Error::new(ErrorKind::Other, "Too many links")),
        errno::MSGSIZE => Err(Error::new(ErrorKind::Other, "Message too large")),
        errno::MULTIHOP => Err(Error::new(ErrorKind::Other, "Reserved")),
        errno::NAMETOOLONG => Err(Error::new(ErrorKind::Other, "Filename too long")),
        errno::NETDOWN => Err(Error::new(ErrorKind::Other, "Network is down")),
        errno::NETRESET => Err(Error::new(ErrorKind::Other, "Connection aborted by network")),
        errno::NETUNREACH => Err(Error::new(ErrorKind::Other, "Network unreachable")),
        errno::NFILE => Err(Error::new(ErrorKind::Other, "Too many files open in system")),
        errno::NOBUFS => Err(Error::new(ErrorKind::Other, "No buffer space available")),
        errno::NODEV => Err(Error::new(ErrorKind::Other, "No such device")),
        errno::NOENT => Err(Error::new(ErrorKind::NotFound, "No such file or directory")),
        errno::NOEXEC => Err(Error::new(ErrorKind::Other, "Executable file format error")),
        errno::NOLCK => Err(Error::new(ErrorKind::Other, "No locks available")),
        errno::NOLINK => Err(Error::new(ErrorKind::Other, "Reserved")),
        errno::NOMEM => Err(Error::new(ErrorKind::Other, "Not enough space")),
        errno::NOMSG => Err(Error::new(ErrorKind::Other, "No message of the desired type")),
        errno::NOPROTOOPT => Err(Error::new(ErrorKind::Other, "Protocol not available")),
        errno::NOSPC => Err(Error::new(ErrorKind::Other, "No space left on device")),
        errno::NOSYS => Err(Error::new(ErrorKind::Other, "Function not supported")),
        errno::NOTCONN => Err(Error::new(ErrorKind::Other, "The socket is not connected")),
        errno::NOTDIR => Err(Error::new(ErrorKind::Other,
                                        "Not a directory or a symbolic link to a directory")),
        errno::NOTEMPTY => Err(Error::new(ErrorKind::Other, "Directory not empty")),
        errno::NOTRECOVERABLE => Err(Error::new(ErrorKind::Other, "State not recoverable")),
        errno::NOTSOCK => Err(Error::new(ErrorKind::Other, "Not a socket")),
        errno::NOTSUP => Err(Error::new(ErrorKind::Other,
                                        "Not supported, or operation not supported on socket")),
        errno::NOTTY => Err(Error::new(ErrorKind::Other, "Inappropriate I/O control operation")),
        errno::NXIO => Err(Error::new(ErrorKind::Other, "No such device or address")),
        errno::OVERFLOW => Err(Error::new(ErrorKind::Other,
                                          "Value too large to be stored in data type")),
        errno::OWNERDEAD => Err(Error::new(ErrorKind::Other, "Previous owner died")),
        errno::PERM => Err(Error::new(ErrorKind::Other, "Operation not permitted")),
        errno::PIPE => Err(Error::new(ErrorKind::BrokenPipe, "Broken pipe")),
        errno::PROTO => Err(Error::new(ErrorKind::Other, "Protocol error")),
        errno::PROTONOSUPPORT => Err(Error::new(ErrorKind::Other, "Protocol not supported")),
        errno::PROTOTYPE => Err(Error::new(ErrorKind::Other, "Protocol wrong type for socket")),
        errno::RANGE => Err(Error::new(ErrorKind::Other, "Result too large")),
        errno::ROFS => Err(Error::new(ErrorKind::Other, "Read-only file system")),
        errno::SPIPE => Err(Error::new(ErrorKind::Other, "Invalid seek")),
        errno::SRCH => Err(Error::new(ErrorKind::Other, "No such process")),
        errno::STALE => Err(Error::new(ErrorKind::Other, "Reserved")),
        errno::TIMEDOUT => Err(Error::new(ErrorKind::TimedOut, "Connection timed out")),
        errno::TXTBSY => Err(Error::new(ErrorKind::Other, "Text file busy")),
        errno::XDEV => Err(Error::new(ErrorKind::Other, "Cross-device link")),
        errno::NOTCAPABLE => Err(Error::new(ErrorKind::Other,
                                            "Extension: Capabilities insufficient")),
    }
}
