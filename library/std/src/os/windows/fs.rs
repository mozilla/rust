//! Windows-specific extensions for the primitives in the `std::fs` module.

#![stable(feature = "rust1", since = "1.0.0")]

use crate::fs::{self, Metadata, OpenOptions};
use crate::io;
use crate::path::Path;
use crate::sys;
use crate::sys_common::{AsInner, AsInnerMut};

/// Windows-specific extensions to [`fs::File`].
#[stable(feature = "file_offset", since = "1.15.0")]
pub trait FileExt {
    /// Seeks to a given position and reads a number of bytes.
    ///
    /// Returns the number of bytes read.
    ///
    /// The offset is relative to the start of the file and thus independent
    /// from the current cursor. The current cursor **is** affected by this
    /// function, it is set to the end of the read.
    ///
    /// Reading beyond the end of the file will always return with a length of
    /// 0\.
    ///
    /// Note that similar to `File::read`, it is not an error to return with a
    /// short read. When returning from such a short read, the file pointer is
    /// still updated.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs::File;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///     let mut buffer = [0; 10];
    ///
    ///     // Read 10 bytes, starting 72 bytes from the
    ///     // start of the file.
    ///     file.seek_read(&mut buffer[..], 72)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_offset", since = "1.15.0")]
    fn seek_read(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;

    /// Seeks to a given position and writes a number of bytes.
    ///
    /// Returns the number of bytes written.
    ///
    /// The offset is relative to the start of the file and thus independent
    /// from the current cursor. The current cursor **is** affected by this
    /// function, it is set to the end of the write.
    ///
    /// When writing beyond the end of the file, the file is appropriately
    /// extended and the intermediate bytes are left uninitialized.
    ///
    /// Note that similar to `File::write`, it is not an error to return a
    /// short write. When returning from such a short write, the file pointer
    /// is still updated.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     // Write a byte string starting 72 bytes from
    ///     // the start of the file.
    ///     buffer.seek_write(b"some bytes", 72)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_offset", since = "1.15.0")]
    fn seek_write(&self, buf: &[u8], offset: u64) -> io::Result<usize>;
}

#[stable(feature = "file_offset", since = "1.15.0")]
impl FileExt for fs::File {
    fn seek_read(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.as_inner().read_at(buf, offset)
    }

    fn seek_write(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        self.as_inner().write_at(buf, offset)
    }
}

/// Windows-specific extensions to [`fs::OpenOptions`].
#[stable(feature = "open_options_ext", since = "1.10.0")]
pub trait OpenOptionsExt {
    /// Overrides the `dwDesiredAccess` argument to the call to [`CreateFile`]
    /// with the specified value.
    ///
    /// This will override the `read`, `write`, and `append` flags on the
    /// `OpenOptions` structure. This method provides fine-grained control over
    /// the permissions to read, write and append data, attributes (like hidden
    /// and system), and extended attributes.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    /// use std::os::windows::prelude::*;
    ///
    /// // Open without read and write permission, for example if you only need
    /// // to call `stat` on the file
    /// let file = OpenOptions::new().access_mode(0).open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn access_mode(&mut self, access: u32) -> &mut Self;

    /// Overrides the `dwShareMode` argument to the call to [`CreateFile`] with
    /// the specified value.
    ///
    /// By default `share_mode` is set to
    /// `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`. This allows
    /// other processes to read, write, and delete/rename the same file
    /// while it is open. Removing any of the flags will prevent other
    /// processes from performing the corresponding operation until the file
    /// handle is closed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    /// use std::os::windows::prelude::*;
    ///
    /// // Do not allow others to read or modify this file while we have it open
    /// // for writing.
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .share_mode(0)
    ///     .open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn share_mode(&mut self, val: u32) -> &mut Self;

    /// Sets extra flags for the `dwFileFlags` argument to the call to
    /// [`CreateFile2`] to the specified value (or combines it with
    /// `attributes` and `security_qos_flags` to set the `dwFlagsAndAttributes`
    /// for [`CreateFile`]).
    ///
    /// Custom flags can only set flags, not remove flags set by Rust's options.
    /// This option overwrites any previously set custom flags.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(for_demonstration_only)]
    /// extern crate winapi;
    /// # mod winapi { pub const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x04000000; }
    ///
    /// use std::fs::OpenOptions;
    /// use std::os::windows::prelude::*;
    ///
    /// let file = OpenOptions::new()
    ///     .create(true)
    ///     .write(true)
    ///     .custom_flags(winapi::FILE_FLAG_DELETE_ON_CLOSE)
    ///     .open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn custom_flags(&mut self, flags: u32) -> &mut Self;

    /// Sets the `dwFileAttributes` argument to the call to [`CreateFile2`] to
    /// the specified value (or combines it with `custom_flags` and
    /// `security_qos_flags` to set the `dwFlagsAndAttributes` for
    /// [`CreateFile`]).
    ///
    /// If a _new_ file is created because it does not yet exist and
    /// `.create(true)` or `.create_new(true)` are specified, the new file is
    /// given the attributes declared with `.attributes()`.
    ///
    /// If an _existing_ file is opened with `.create(true).truncate(true)`, its
    /// existing attributes are preserved and combined with the ones declared
    /// with `.attributes()`.
    ///
    /// In all other cases the attributes get ignored.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(for_demonstration_only)]
    /// extern crate winapi;
    /// # mod winapi { pub const FILE_ATTRIBUTE_HIDDEN: u32 = 2; }
    ///
    /// use std::fs::OpenOptions;
    /// use std::os::windows::prelude::*;
    ///
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .create(true)
    ///     .attributes(winapi::FILE_ATTRIBUTE_HIDDEN)
    ///     .open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn attributes(&mut self, val: u32) -> &mut Self;

    /// Sets the `dwSecurityQosFlags` argument to the call to [`CreateFile2`] to
    /// the specified value (or combines it with `custom_flags` and `attributes`
    /// to set the `dwFlagsAndAttributes` for [`CreateFile`]).
    ///
    /// By default `security_qos_flags` is not set. It should be specified when
    /// opening a named pipe, to control to which degree a server process can
    /// act on behalf of a client process (security impersonation level).
    ///
    /// When `security_qos_flags` is not set, a malicious program can gain the
    /// elevated privileges of a privileged Rust process when it allows opening
    /// user-specified paths, by tricking it into opening a named pipe. So
    /// arguably `security_qos_flags` should also be set when opening arbitrary
    /// paths. However the bits can then conflict with other flags, specifically
    /// `FILE_FLAG_OPEN_NO_RECALL`.
    ///
    /// For information about possible values, see [Impersonation Levels] on the
    /// Windows Dev Center site. The `SECURITY_SQOS_PRESENT` flag is set
    /// automatically when using this method.

    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(for_demonstration_only)]
    /// extern crate winapi;
    /// # mod winapi { pub const SECURITY_IDENTIFICATION: u32 = 0; }
    /// use std::fs::OpenOptions;
    /// use std::os::windows::prelude::*;
    ///
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .create(true)
    ///
    ///     // Sets the flag value to `SecurityIdentification`.
    ///     .security_qos_flags(winapi::SECURITY_IDENTIFICATION)
    ///
    ///     .open(r"\\.\pipe\MyPipe");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
    /// [Impersonation Levels]:
    ///     https://docs.microsoft.com/en-us/windows/win32/api/winnt/ne-winnt-security_impersonation_level
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn security_qos_flags(&mut self, flags: u32) -> &mut Self;
}

#[stable(feature = "open_options_ext", since = "1.10.0")]
impl OpenOptionsExt for OpenOptions {
    fn access_mode(&mut self, access: u32) -> &mut OpenOptions {
        self.as_inner_mut().access_mode(access);
        self
    }

    fn share_mode(&mut self, share: u32) -> &mut OpenOptions {
        self.as_inner_mut().share_mode(share);
        self
    }

    fn custom_flags(&mut self, flags: u32) -> &mut OpenOptions {
        self.as_inner_mut().custom_flags(flags);
        self
    }

    fn attributes(&mut self, attributes: u32) -> &mut OpenOptions {
        self.as_inner_mut().attributes(attributes);
        self
    }

    fn security_qos_flags(&mut self, flags: u32) -> &mut OpenOptions {
        self.as_inner_mut().security_qos_flags(flags);
        self
    }
}

/// Windows-specific extensions to [`fs::Metadata`].
///
/// The members of this trait correspond to metadata exposed by calls to either
/// [`GetFileInformationByHandle`] or [`GetFileInformationByHandleEx`].
///
/// [`GetFileInformationByHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
/// [`GetFileInformationByHandleEx`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
#[stable(feature = "metadata_ext", since = "1.1.0")]
pub trait MetadataExt {
    /// Returns the file attributes of a file or directory;
    /// corresponds to the `dwFileAttributes` field returned by [`GetFileInformationByHandle`],
    /// or the `FileAttributes` field returned by [`GetFileInformationByHandleEx`].
    ///
    /// For possible values and their descriptions, see [File Attribute Constants] in the Windows Dev Center.
    ///
    /// [`GetFileInformationByHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
    /// [`GetFileInformationByHandleEx`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
    /// [File Attribute Constants]: https://docs.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let attributes = metadata.file_attributes();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn file_attributes(&self) -> u32;

    /// Returns the creation time of a file or directory;
    /// corresponds to the `ftCreationTime` field returned by [`GetFileInformationByHandle`],
    /// or the `CreationTime` field returned by [`GetFileInformationByHandleEx`].
    ///
    /// The returned 64-bit value is equivalent to a [`FILETIME`] struct,
    /// which represents the number of 100-nanosecond intervals since
    /// January 1, 1601 (UTC). The struct is automatically
    /// converted to a `u64` value, as that is the recommended way
    /// to use it.
    ///
    /// If the underlying filesystem does not support creation time, the
    /// returned value is 0.
    ///
    /// [`GetFileInformationByHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
    /// [`GetFileInformationByHandleEx`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
    /// [`FILETIME`]: https://docs.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-filetime
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let creation_time = metadata.creation_time();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn creation_time(&self) -> u64;

    /// Returns the last access time of a file or directory;
    /// corresponds to the `ftLastAccessTime` field returned by [`GetFileInformationByHandle`],
    /// or the `LastAccessTime` field returned by [`GetFileInformationByHandleEx`].
    ///
    /// The returned 64-bit value is equivalent to a [`FILETIME`] struct,
    /// which represents the number of 100-nanosecond intervals since
    /// January 1, 1601 (UTC). The struct is automatically
    /// converted to a `u64` value, as that is the recommended way
    /// to use it.
    ///
    /// For a file, the value specifies the last time that a file was read
    /// from or written to. For a directory, the value specifies when
    /// the directory was created. For both files and directories, the
    /// specified date is correct, but the time of day is always set to
    /// midnight.
    ///
    /// If the underlying filesystem does not support last access time, the
    /// returned value is 0.
    ///
    /// [`GetFileInformationByHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
    /// [`GetFileInformationByHandleEx`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
    /// [`FILETIME`]: https://docs.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-filetime
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let last_access_time = metadata.last_access_time();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn last_access_time(&self) -> u64;

    /// Returns the last write time of a file or directory;
    /// corresponds to the `ftLastWriteTime` field returned by [`GetFileInformationByHandle`],
    /// or the `LastWriteTime` field returned by [`GetFileInformationByHandleEx`].
    ///
    /// The returned 64-bit value is equivalent to a [`FILETIME`] struct,
    /// which represents the number of 100-nanosecond intervals since
    /// January 1, 1601 (UTC). The struct is automatically
    /// converted to a `u64` value, as that is the recommended way
    /// to use it.
    ///
    /// For a file, the value specifies the last time that a file was written
    /// to. For a directory, the structure specifies when the directory was
    /// created.
    ///
    /// If the underlying filesystem does not support the last write time,
    /// the returned value is 0.
    ///
    /// [`GetFileInformationByHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
    /// [`GetFileInformationByHandleEx`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
    /// [`FILETIME`]: https://docs.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-filetime
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let last_write_time = metadata.last_write_time();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn last_write_time(&self) -> u64;

    /// Returns the size of a file;
    /// corresponds to the `nFileSize{High,Low}` fields returned by [`GetFileInformationByHandle`],
    /// or the `AllocationSize` field returned by [`GetFileInformationByHandleEx`].
    ///
    /// The returned value does not have meaning for directories.
    ///
    /// [`GetFileInformationByHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
    /// [`GetFileInformationByHandleEx`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let file_size = metadata.file_size();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn file_size(&self) -> u64;

    /// Returns the volume serial number of a file or directory;
    /// corresponds to the `dwVolumeSerialNumber` field returned by [`GetFileInformationByHandle`].
    ///
    /// This will return `None` if the `Metadata` instance was created from a
    /// call to `DirEntry::metadata`. If this `Metadata` was created by using
    /// `fs::metadata` or `File::metadata`, then this will return `Some`.
    ///
    /// [`GetFileInformationByHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
    #[unstable(feature = "windows_by_handle", issue = "63010")]
    fn volume_serial_number(&self) -> Option<u32>;

    /// Returns the number of links to a file or directory;
    /// corresponds to the `nNumberOfLinks` field returned by [`GetFileInformationByHandle`],
    /// or the `NumberOfLinks` field returned by [`GetFileInformationByHandleEx`].
    ///
    /// This will return `None` if the `Metadata` instance was created from a
    /// call to `DirEntry::metadata`. If this `Metadata` was created by using
    /// `fs::metadata` or `File::metadata`, then this will return `Some`.
    ///
    /// [`GetFileInformationByHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
    /// [`GetFileInformationByHandleEx`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
    #[unstable(feature = "windows_by_handle", issue = "63010")]
    fn number_of_links(&self) -> Option<u32>;

    /// Returns the file index of a file or directory;
    /// corresponds to the `nFileIndex{Low,High}` fields returned by [`GetFileInformationByHandle`].
    ///
    /// This will return `None` if the `Metadata` instance was created from a
    /// call to `DirEntry::metadata`. If this `Metadata` was created by using
    /// `fs::metadata` or `File::metadata`, then this will return `Some`.
    ///
    /// [`GetFileInformationByHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
    #[unstable(feature = "windows_by_handle", issue = "63010")]
    fn file_index(&self) -> Option<u64>;
}

#[stable(feature = "metadata_ext", since = "1.1.0")]
impl MetadataExt for Metadata {
    fn file_attributes(&self) -> u32 {
        self.as_inner().attrs()
    }
    fn creation_time(&self) -> u64 {
        self.as_inner().created_u64()
    }
    fn last_access_time(&self) -> u64 {
        self.as_inner().accessed_u64()
    }
    fn last_write_time(&self) -> u64 {
        self.as_inner().modified_u64()
    }
    fn file_size(&self) -> u64 {
        self.as_inner().size()
    }
    fn volume_serial_number(&self) -> Option<u32> {
        self.as_inner().volume_serial_number()
    }
    fn number_of_links(&self) -> Option<u32> {
        self.as_inner().number_of_links()
    }
    fn file_index(&self) -> Option<u64> {
        self.as_inner().file_index()
    }
}

/// Windows-specific extensions to [`fs::FileType`].
///
/// On Windows, a symbolic link knows whether it is a file or directory.
#[unstable(feature = "windows_file_type_ext", issue = "none")]
pub trait FileTypeExt {
    /// Returns `true` if this file type is a symbolic link that is also a directory.
    #[unstable(feature = "windows_file_type_ext", issue = "none")]
    fn is_symlink_dir(&self) -> bool;
    /// Returns `true` if this file type is a symbolic link that is also a file.
    #[unstable(feature = "windows_file_type_ext", issue = "none")]
    fn is_symlink_file(&self) -> bool;
}

#[unstable(feature = "windows_file_type_ext", issue = "none")]
impl FileTypeExt for fs::FileType {
    fn is_symlink_dir(&self) -> bool {
        self.as_inner().is_symlink_dir()
    }
    fn is_symlink_file(&self) -> bool {
        self.as_inner().is_symlink_file()
    }
}

/// Creates a new file symbolic link on the filesystem.
///
/// The `link` path will be a file symbolic link pointing to the `original`
/// path.
///
/// # Examples
///
/// ```no_run
/// use std::os::windows::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::symlink_file("a.txt", "b.txt")?;
///     Ok(())
/// }
/// ```
#[stable(feature = "symlink", since = "1.1.0")]
pub fn symlink_file<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> io::Result<()> {
    sys::fs::symlink_inner(original.as_ref(), link.as_ref(), false)
}

/// Creates a new directory symlink on the filesystem.
///
/// The `link` path will be a directory symbolic link pointing to the `original`
/// path.
///
/// # Examples
///
/// ```no_run
/// use std::os::windows::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::symlink_dir("a", "b")?;
///     Ok(())
/// }
/// ```
#[stable(feature = "symlink", since = "1.1.0")]
pub fn symlink_dir<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> io::Result<()> {
    sys::fs::symlink_inner(original.as_ref(), link.as_ref(), true)
}
