// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;
use io::prelude::*;

use io;
use libc::{self, c_int, size_t, c_void};
use mem;
use sys::cvt;
use sys_common::AsInner;

pub struct FileDesc {
    fd: c_int,
}

impl FileDesc {
    pub fn new(fd: c_int) -> FileDesc {
        FileDesc { fd: fd }
    }

    pub fn close(self) -> io::Result<()> {
        let fd = self.fd;
        // Don't run the destructor after running the `close` function.
        unsafe { mem::forget(self) };

        // Closing stdio file handles makes no sense, so never do it. Note that
        // this is relied upon in the destructor.
        if fd <= libc::STDERR_FILENO {
            return Ok(());
        }

        // Also, note that closing the file descriptor is never retried on
        // error. The reason for this is that if an error occurs, we don't
        // actually know if the file descriptor was closed or not, and if we
        // retried (for something like EINTR), we might close another valid
        // file descriptor (opened after we closed ours).
        try!(cvt(unsafe { libc::close(fd) }));
        Ok(())
    }

    pub fn raw(&self) -> c_int { self.fd }

    /// Extract the actual filedescriptor without closing it.
    pub fn into_raw(self) -> c_int {
        let fd = self.fd;
        unsafe { mem::forget(self) };
        fd
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let ret = try!(cvt(unsafe {
            libc::read(self.fd,
                       buf.as_mut_ptr() as *mut c_void,
                       buf.len() as size_t)
        }));
        Ok(ret as usize)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let ret = try!(cvt(unsafe {
            libc::write(self.fd,
                        buf.as_ptr() as *const c_void,
                        buf.len() as size_t)
        }));
        Ok(ret as usize)
    }
}

impl AsInner<c_int> for FileDesc {
    fn as_inner(&self) -> &c_int { &self.fd }
}

impl Drop for FileDesc {
    fn drop(&mut self) {
        // Close the file descriptor.
        let fd = mem::replace(self, FileDesc::new(libc::STDIN_FILENO));
        fd.close().unwrap();
    }
}
