// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Synchronous, in-memory pipes.
//!
//! Currently these aren't particularly useful, there only exists bindings
//! enough so that pipes can be created to child processes.

use prelude::*;
use super::{Reader, Writer};
use io::IoResult;
use io::native::file;
use rt::rtio::{RtioPipe, with_local_io};

pub struct PipeStream {
    priv obj: ~RtioPipe,
}

impl PipeStream {
    /// Consumes a file descriptor to return a pipe stream that will have
    /// synchronous, but non-blocking reads/writes. This is useful if the file
    /// descriptor is acquired via means other than the standard methods.
    ///
    /// This operation consumes ownership of the file descriptor and it will be
    /// closed once the object is deallocated.
    ///
    /// # Example
    ///
    ///     use std::libc;
    ///     use std::io::pipe;
    ///
    ///     let mut pipe = PipeStream::open(libc::STDERR_FILENO);
    ///     pipe.write(bytes!("Hello, stderr!"));
    ///
    /// # Failure
    ///
    /// If the pipe cannot be created, an error will be raised on the
    /// `io_error` condition.
    pub fn open(fd: file::fd_t) -> IoResult<PipeStream> {
        with_local_io(|io| io.pipe_open(fd).map(|obj| PipeStream { obj: obj }))
    }

    pub fn new(inner: ~RtioPipe) -> PipeStream {
        PipeStream { obj: inner }
    }
}

impl Reader for PipeStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> { self.obj.read(buf) }
    fn eof(&mut self) -> bool { false }
}

impl Writer for PipeStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> { self.obj.write(buf) }
}
