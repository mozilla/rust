use crate::fmt::{self, Debug, Formatter};
use crate::io::{buffered::LineWriterShim, BufWriter, Write};
/// Different buffering modes a writer can use
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BufferMode {
    /// Unbuffered: forward writes directly to the underlying writer. In some
    /// cases, a writer may buffer temporarily (for instance, when processing
    /// a formatted write), but even in this case it will always immediately
    /// flush.
    None,

    /// Block buffering: buffer writes until the buffer is full, then forward
    /// to the underlying writer
    Block,

    /// Line buffering: same as block buffering, except that it immediately
    /// forwards the buffered content when it encounters a newline.
    Line,
}

/// Wraps a writer and provides a switchable buffering mode for its output
#[derive(Debug)]
pub struct SwitchWriter<W: Write> {
    buffer: BufWriter<W>,
    mode: BufferMode,
}

impl<W: Write> SwitchWriter<W> {
    pub fn with_capacity(capacity: usize, writer: W, mode: BufferMode) -> Self {
        Self { buffer: BufWriter::with_capacity(capacity, writer), mode }
    }

    pub fn new(writer: W, mode: BufferMode) {
        Self { buffer: BufWriter::new(writer), mode }
    }

    pub fn mode(&self) -> BufferMode {
        self.mode
    }

    /// Set the buffering mode. This will not attempt any io; it only changes
    /// the mode used for subsequent writes.
    pub fn set_mode(&mut self, mode: BufferMode) {
        self.mode = mode
    }
}

/// Shared logic for io methods that need to switch over the buffering mode
macro_rules! use_correct_writer {
    ($this:ident, |$writer:ident| $usage:expr) => {
        match $this.mode {
            BufferMode::None => {
                $this.buffer.flush_buf()?;
                let $writer = self.buffer.get_mut();
                $usage
            }
            BufferMode::Block => {
                let $writer = &mut self.buffer;
                $usage
            }
            BufferMode::Line => {
                let shim = LineWriterShim::new(&mut self.buffer);
                $usage
            }
        }
    };
}

impl<W: Write> Write for SwitchWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use_correct_writer!(self, |writer| writer.write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buffer.flush()
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        use_correct_writer!(self, |writer| writer.write_vectored(bufs))
    }

    fn is_write_vectored(&self) -> bool {
        self.buffer.is_write_vectored()
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        use_correct_writer!(self, |writer| writer.write_all(buf))
    }

    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> Result<()> {
        use_correct_writer!(self, |writer| writer.write_all_vectored(bufs))
    }

    fn write_fmt(&mut self, fmt: Arguments<'_>) -> Result<()> {
        match self.mode {
            BufferMode::None => {
                // write_fmt is usually going to be very numerous tiny writes
                // from the constituent writers, so even though we're in
                // unbuffered mode we still collect it to the buffer so that
                // we can flush it in a single write.
                self.buffer.flush_buf()?;
                self.buffer.write_fmt(fmt)?;
                self.buffer.flush_buf()
            }
            BufferMode::Block => self.buffer.write_fmt(fmt),
            BufferMode::Line => {
                let shim = LineWriterShim::new(&mut self.buffer);
                shim.write_fmt(fmt)
            }
        }
    }
}
