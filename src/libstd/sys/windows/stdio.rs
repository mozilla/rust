#![unstable(issue = "0", feature = "windows_stdio")]

use char::decode_utf16;
use cmp;
use io;
use ptr;
use str;
use sys::c;
use sys::cvt;
use sys::handle::Handle;

// Don't cache handles but get them fresh for every read/write. This allows us to track changes to
// the value over time (such as if a process calls `SetStdHandle` while it's running). See #40490.
pub struct Stdin {
    surrogate: u16,
}
pub struct Stdout;
pub struct Stderr;

// Apparently Windows doesn't handle large reads on stdin or writes to stdout/stderr well (see
// #13304 for details).
//
// From MSDN (2011): "The storage for this buffer is allocated from a shared heap for the
// process that is 64 KB in size. The maximum size of the buffer will depend on heap usage."
//
// We choose the cap at 8 KiB because libuv does the same, and it seems to be acceptable so far.
const MAX_BUFFER_SIZE: usize = 8192;

// The standard buffer size of BufReader for Stdin should be able to hold 3x more bytes than there
// are `u16`'s in MAX_BUFFER_SIZE. This ensures the read data can always be completely decoded from
// UTF-16 to UTF-8.
pub const STDIN_BUF_SIZE: usize = MAX_BUFFER_SIZE / 2 * 3;

pub fn get_handle(handle_id: c::DWORD) -> io::Result<c::HANDLE> {
    let handle = unsafe { c::GetStdHandle(handle_id) };
    if handle == c::INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else if handle.is_null() {
        Err(io::Error::from_raw_os_error(c::ERROR_INVALID_HANDLE as i32))
    } else {
        Ok(handle)
    }
}

fn is_console(handle: c::HANDLE) -> bool {
    // `GetConsoleMode` will return false (0) if this is a pipe (we don't care about the reported
    // mode). This will only detect Windows Console, not other terminals connected to a pipe like
    // MSYS. Which is exactly what we need, as only Windows Console needs a conversion to UTF-16.
    let mut mode = 0;
    unsafe { c::GetConsoleMode(handle, &mut mode) != 0 }
}

fn write(handle_id: c::DWORD, data: &[u8]) -> io::Result<usize> {
    let handle = get_handle(handle_id)?;
    if !is_console(handle) {
        let handle = Handle::new(handle);
        let ret = handle.write(data);
        handle.into_raw(); // Don't close the handle
        return ret;
    }

    // As the console is meant for presenting text, we assume bytes of `data` come from a string
    // and are encoded as UTF-8, which needs to be encoded as UTF-16.
    //
    // If the data is not valid UTF-8 we write out as many bytes as are valid.
    // Only when there are no valid bytes (which will happen on the next call), return an error.
    let len = cmp::min(data.len(), MAX_BUFFER_SIZE / 2);
    let utf8 = match str::from_utf8(&data[..len]) {
        Ok(s) => s,
        Err(ref e) if e.valid_up_to() == 0 => {
            return Err(io::Error::new(io::ErrorKind::InvalidData,
                "Windows stdio in console mode does not support writing non-UTF-8 byte sequences"))
        },
        Err(e) => str::from_utf8(&data[..e.valid_up_to()]).unwrap(),
    };
    let mut utf16 = [0u16; MAX_BUFFER_SIZE / 2];
    let mut len_utf16 = 0;
    for (chr, dest) in utf8.encode_utf16().zip(utf16.iter_mut()) {
        *dest = chr;
        len_utf16 += 1;
    }
    let utf16 = &utf16[..len_utf16];

    let mut written = write_u16s(handle, &utf16)?;

    // Figure out how many bytes of as UTF-8 were written away as UTF-16.
    if written == utf16.len() {
        Ok(utf8.len())
    } else {
        // Make sure we didn't end up writing only half of a surrogate pair (even though the chance
        // is tiny). Because it is not possible for user code to re-slice `data` in such a way that
        // a missing surrogate can be produced (and also because of the UTF-8 validation above),
        // write the missing surrogate out now.
        // Buffering it would mean we have to lie about the number of bytes written.
        let first_char_remaining = utf16[written];
        if first_char_remaining >= 0xDCEE && first_char_remaining <= 0xDFFF { // low surrogate
            // We just hope this works, and give up otherwise
            let _ = write_u16s(handle, &utf16[written..written+1]);
            written += 1;
        }
        // Calculate the number of bytes of `utf8` that were actually written.
        let mut count = 0;
        for ch in utf16[..written].iter() {
            count += match ch {
                0x0000 ..= 0x007F => 1,
                0x0080 ..= 0x07FF => 2,
                0xDCEE ..= 0xDFFF => 1, // Low surrogate. We already counted 3 bytes for the other.
                _ => 3,
            };
        }
        debug_assert!(String::from_utf16(&utf16[..written]).unwrap() == utf8[..count]);
        Ok(count)
    }
}

fn write_u16s(handle: c::HANDLE, data: &[u16]) -> io::Result<usize> {
    let mut written = 0;
    cvt(unsafe {
        c::WriteConsoleW(handle,
                         data.as_ptr() as c::LPCVOID,
                         data.len() as u32,
                         &mut written,
                         ptr::null_mut())
    })?;
    Ok(written as usize)
}

impl Stdin {
    pub fn new() -> io::Result<Stdin> {
        Ok(Stdin { surrogate: 0 })
    }
}

impl io::Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let handle = get_handle(c::STD_INPUT_HANDLE)?;
        if !is_console(handle) {
            let handle = Handle::new(handle);
            let ret = handle.read(buf);
            handle.into_raw(); // Don't close the handle
            return ret;
        }

        if buf.len() == 0 {
            return Ok(0);
        } else if buf.len() < 4 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                        "Windows stdin in console mode does not support a buffer too small to \
                        guarantee holding one arbitrary UTF-8 character (4 bytes)"))
        }

        let mut utf16_buf = [0u16; MAX_BUFFER_SIZE / 2];
        // In the worst case, an UTF-8 string can take 3 bytes for every `u16` of an UTF-16. So
        // we can read at most a third of `buf.len()` chars and uphold the guarantee no data gets
        // lost.
        let amount = cmp::min(buf.len() / 3, utf16_buf.len());
        let read = read_u16s_fixup_surrogates(handle, &mut utf16_buf, amount, &mut self.surrogate)?;

        utf16_to_utf8(&utf16_buf[..read], buf)
    }
}


// We assume that if the last `u16` is an unpaired surrogate they got sliced apart by our
// buffer size, and keep it around for the next read hoping to put them together.
// This is a best effort, and may not work if we are not the only reader on Stdin.
fn read_u16s_fixup_surrogates(handle: c::HANDLE,
                              buf: &mut [u16],
                              mut amount: usize,
                              surrogate: &mut u16) -> io::Result<usize>
{
    // Insert possibly remaining unpaired surrogate from last read.
    let mut start = 0;
    if *surrogate != 0 {
        buf[0] = *surrogate;
        *surrogate = 0;
        start = 1;
        if amount == 1 {
            // Special case: `Stdin::read` guarantees we can always read at least one new `u16`
            // and combine it with an unpaired surrogate, because the UTF-8 buffer is at least
            // 4 bytes.
            amount = 2;
        }
    }
    let mut amount = read_u16s(handle, &mut buf[start..amount])? + start;

    if amount > 0 {
        let last_char = buf[amount - 1];
        if last_char >= 0xD800 && last_char <= 0xDBFF { // high surrogate
            *surrogate = last_char;
            amount -= 1;
        }
    }
    Ok(amount)
}

fn read_u16s(handle: c::HANDLE, buf: &mut [u16]) -> io::Result<usize> {
    // Configure the `pInputControl` parameter to not only return on `\r\n` but also Ctrl-Z, the
    // traditional DOS method to indicate end of character stream / user input (SUB).
    // See #38274 and https://stackoverflow.com/questions/43836040/win-api-readconsole.
    const CTRL_Z: u16 = 0x1A;
    const CTRL_Z_MASK: c::ULONG = 1 << CTRL_Z;
    let mut input_control = c::CONSOLE_READCONSOLE_CONTROL {
        nLength: ::mem::size_of::<c::CONSOLE_READCONSOLE_CONTROL>() as c::ULONG,
        nInitialChars: 0,
        dwCtrlWakeupMask: CTRL_Z_MASK,
        dwControlKeyState: 0,
    };

    let mut amount = 0;
    cvt(unsafe {
        c::ReadConsoleW(handle,
                        buf.as_mut_ptr() as c::LPVOID,
                        buf.len() as u32,
                        &mut amount,
                        &mut input_control as c::PCONSOLE_READCONSOLE_CONTROL)
    })?;

    if amount > 0 && buf[amount as usize - 1] == CTRL_Z {
        amount -= 1;
    }
    Ok(amount as usize)
}

#[allow(unused)]
fn utf16_to_utf8(utf16: &[u16], utf8: &mut [u8]) -> io::Result<usize> {
    let mut written = 0;
    for chr in decode_utf16(utf16.iter().cloned()) {
        match chr {
            Ok(chr) => {
                chr.encode_utf8(&mut utf8[written..]);
                written += chr.len_utf8();
            }
            Err(_) => {
                // We can't really do any better than forget all data and return an error.
                return Err(io::Error::new(io::ErrorKind::InvalidData,
                    "Windows stdin in console mode does not support non-UTF-16 input; \
                    encountered unpaired surrogate"))
            }
        }
    }
    Ok(written)
}

impl Stdout {
    pub fn new() -> io::Result<Stdout> {
        Ok(Stdout)
    }
}

impl io::Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(c::STD_OUTPUT_HANDLE, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Stderr {
    pub fn new() -> io::Result<Stderr> {
        Ok(Stderr)
    }
}

impl io::Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(c::STD_ERROR_HANDLE, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn is_ebadf(err: &io::Error) -> bool {
    err.raw_os_error() == Some(c::ERROR_INVALID_HANDLE as i32)
}

pub fn panic_output() -> Option<impl io::Write> {
    io::stderr_raw().ok()
}
