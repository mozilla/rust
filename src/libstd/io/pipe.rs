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

#![allow(missing_docs)]

use prelude::*;

use io::IoResult;
use libc;
use sync::Arc;

use sys_common;
use sys;
use sys::fs::FileDesc as FileDesc;

/// A synchronous, in-memory pipe.
pub struct PipeImpl<D> {
    inner: Arc<FileDesc>
}

struct Readable;
struct Writable;

pub type PipeReader = PipeImpl<Readable>;
pub type PipeWriter = PipeImpl<Writable>;

pub struct PipePair {
    pub reader: PipeReader,
    pub writer: PipeWriter,
}

impl PipePair {
    /// Creates a pair of in-memory OS pipes for a unidirectional communication
    /// stream.
    ///
    /// The structure returned contains a reader and writer I/O object. Data
    /// written to the writer can be read from the reader.
    ///
    /// # Errors
    ///
    /// This function can fail to succeed if the underlying OS has run out of
    /// available resources to allocate a new pipe.
    pub fn new() -> IoResult<PipePair> {
        let (reader, writer) = try!(unsafe { sys::os::pipe() });
        Ok(PipePair {
            reader: Pipe::from_filedesc(reader),
            writer: Pipe::from_filedesc(writer),
        })
    }
}

pub struct Pipe;

impl Pipe {
    /// Consumes a file descriptor to return a pipe stream that will have
    /// synchronous, but non-blocking reads/writes. This is useful if the file
    /// descriptor is acquired via means other than the standard methods.
    ///
    /// This operation consumes ownership of the file descriptor and it will be
    /// closed once the object is deallocated.
    ///
    /// # Example
    ///
    /// ```{rust,no_run}
    /// # #![allow(unused_must_use)]
    /// extern crate libc;
    ///
    /// use std::io::pipe::Pipe;
    ///
    /// fn main() {
    ///     let mut pipe = Pipe::open(libc::STDERR_FILENO);
    ///     pipe.write(b"Hello, stderr!");
    /// }
    /// ```
    pub fn open<T>(fd: libc::c_int) -> IoResult<PipeImpl<T>> {
        Ok(Pipe::from_filedesc(FileDesc::new(fd, true)))
    }

    // FIXME: expose this some other way
    /// Wrap a FileDesc directly, taking ownership.
    #[doc(hidden)]
    pub fn from_filedesc<T>(fd: FileDesc) -> PipeImpl<T> {
        PipeImpl { inner: Arc::new(fd) }
    }
}

impl<T> sys_common::AsInner<sys::fs::FileDesc> for PipeImpl<T> {
    fn as_inner(&self) -> &sys::fs::FileDesc {
        &*self.inner
    }
}

impl<T> Clone for PipeImpl<T> {
    fn clone(&self) -> PipeImpl<T> {
        PipeImpl { inner: self.inner.clone() }
    }
}

impl Reader for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.inner.read(buf)
    }
}

impl Writer for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.inner.write(buf)
    }
}

#[cfg(test)]
mod test {
    use prelude::*;

    #[test]
    fn partial_read() {
        use os;
        use io::pipe::Pipe;

        let os::Pipe { reader, writer } = unsafe { os::pipe().unwrap() };
        let out = Pipe::open(writer);
        let mut input = Pipe::open(reader);
        let (tx, rx) = channel();
        spawn(proc() {
            let mut out = out;
            out.write(&[10]).unwrap();
            rx.recv(); // don't close the pipe until the other read has finished
        });

        let mut buf = [0, ..10];
        input.read(&mut buf).unwrap();
        tx.send(());
    }
}
