use crate::ffi::{OsStr, OsString};
use crate::fmt;
use crate::io::{self, IoSlice, IoSliceMut, SeekFrom};
use crate::iter;
use crate::mem::{self, ManuallyDrop};
use crate::os::wasi::ffi::{OsStrExt, OsStringExt};
use crate::path::{Path, PathBuf};
use crate::ptr;
use crate::sync::Arc;
use crate::sys::fd::WasiFd;
use crate::sys::time::SystemTime;
use crate::sys::unsupported;
use crate::sys_common::FromInner;

pub use crate::sys_common::fs::copy;
pub use crate::sys_common::fs::remove_dir_all;

use ::wasi::wasi_unstable as wasi;

pub struct File {
    fd: WasiFd,
}

#[derive(Clone)]
pub struct FileAttr {
    meta: wasi::FileStat,
}

pub struct ReadDir {
    inner: Arc<ReadDirInner>,
    cookie: Option<wasi::DirCookie>,
    buf: Vec<u8>,
    offset: usize,
    cap: usize,
}

struct ReadDirInner {
    root: PathBuf,
    dir: File,
}

pub struct DirEntry {
    meta: wasi::Dirent,
    name: Vec<u8>,
    inner: Arc<ReadDirInner>,
}

#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    dirflags: wasi::LookupFlags,
    fdflags: wasi::FdFlags,
    oflags: wasi::OFlags,
    rights_base: Option<wasi::Rights>,
    rights_inheriting: Option<wasi::Rights>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FilePermissions {
    readonly: bool,
}

#[derive(PartialEq, Eq, Hash, Debug, Copy, Clone)]
pub struct FileType {
    bits: wasi::FileType,
}

#[derive(Debug)]
pub struct DirBuilder {}

impl FileAttr {
    pub fn size(&self) -> u64 {
        self.meta.st_size
    }

    pub fn perm(&self) -> FilePermissions {
        // not currently implemented in wasi yet
        FilePermissions { readonly: false }
    }

    pub fn file_type(&self) -> FileType {
        FileType {
            bits: self.meta.st_filetype,
        }
    }

    pub fn modified(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::from_wasi_timestamp(self.meta.st_mtim))
    }

    pub fn accessed(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::from_wasi_timestamp(self.meta.st_atim))
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::from_wasi_timestamp(self.meta.st_ctim))
    }

    pub fn as_wasi(&self) -> &wasi::FileStat {
        &self.meta
    }
}

impl FilePermissions {
    pub fn readonly(&self) -> bool {
        self.readonly
    }

    pub fn set_readonly(&mut self, readonly: bool) {
        self.readonly = readonly;
    }
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.bits == wasi::FILETYPE_DIRECTORY
    }

    pub fn is_file(&self) -> bool {
        self.bits == wasi::FILETYPE_REGULAR_FILE
    }

    pub fn is_symlink(&self) -> bool {
        self.bits == wasi::FILETYPE_SYMBOLIC_LINK
    }

    pub fn bits(&self) -> wasi::FileType {
        self.bits
    }
}

impl fmt::Debug for ReadDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadDir").finish()
    }
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        loop {
            // If we've reached the capacity of our buffer then we need to read
            // some more from the OS, otherwise we pick up at our old offset.
            let offset = if self.offset == self.cap {
                let cookie = self.cookie.take()?;
                match self.inner.dir.fd.readdir(&mut self.buf, cookie) {
                    Ok(bytes) => self.cap = bytes,
                    Err(e) => return Some(Err(e)),
                }
                self.offset = 0;
                self.cookie = Some(cookie);

                // If we didn't actually read anything, this is in theory the
                // end of the directory.
                if self.cap == 0 {
                    self.cookie = None;
                    return None;
                }

                0
            } else {
                self.offset
            };
            let data = &self.buf[offset..self.cap];

            // If we're not able to read a directory entry then that means it
            // must have been truncated at the end of the buffer, so reset our
            // offset so we can go back and reread into the buffer, picking up
            // where we last left off.
            let dirent_size = mem::size_of::<wasi::Dirent>();
            if data.len() < dirent_size {
                assert!(self.cookie.is_some());
                assert!(self.buf.len() >= dirent_size);
                self.offset = self.cap;
                continue;
            }
            let (dirent, data) = data.split_at(dirent_size);
            let dirent =
                unsafe { ptr::read_unaligned(dirent.as_ptr() as *const wasi::Dirent) };

            // If the file name was truncated, then we need to reinvoke
            // `readdir` so we truncate our buffer to start over and reread this
            // descriptor. Note that if our offset is 0 that means the file name
            // is massive and we need a bigger buffer.
            if data.len() < dirent.d_namlen as usize {
                if offset == 0 {
                    let amt_to_add = self.buf.capacity();
                    self.buf.extend(iter::repeat(0).take(amt_to_add));
                }
                assert!(self.cookie.is_some());
                self.offset = self.cap;
                continue;
            }
            self.cookie = Some(dirent.d_next);
            self.offset = offset + dirent_size + dirent.d_namlen as usize;

            let name = &data[..(dirent.d_namlen as usize)];

            // These names are skipped on all other platforms, so let's skip
            // them here too
            if name == b"." || name == b".." {
                continue;
            }

            return Some(Ok(DirEntry {
                meta: dirent,
                name: name.to_vec(),
                inner: self.inner.clone(),
            }));
        }
    }
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        let name = OsStr::from_bytes(&self.name);
        self.inner.root.join(name)
    }

    pub fn file_name(&self) -> OsString {
        OsString::from_vec(self.name.clone())
    }

    pub fn metadata(&self) -> io::Result<FileAttr> {
        metadata_at(
            &self.inner.dir.fd,
            0,
            OsStr::from_bytes(&self.name).as_ref(),
        )
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(FileType {
            bits: self.meta.d_type,
        })
    }

    pub fn ino(&self) -> wasi::Inode {
        self.meta.d_ino
    }
}

impl OpenOptions {
    pub fn new() -> OpenOptions {
        let mut base = OpenOptions::default();
        base.dirflags = wasi::LOOKUP_SYMLINK_FOLLOW;
        return base;
    }

    pub fn read(&mut self, read: bool) {
        self.read = read;
    }

    pub fn write(&mut self, write: bool) {
        self.write = write;
    }

    pub fn truncate(&mut self, truncate: bool) {
        self.oflag(wasi::O_TRUNC, truncate);
    }

    pub fn create(&mut self, create: bool) {
        self.oflag(wasi::O_CREAT, create);
    }

    pub fn create_new(&mut self, create_new: bool) {
        self.oflag(wasi::O_EXCL, create_new);
        self.oflag(wasi::O_CREAT, create_new);
    }

    pub fn directory(&mut self, directory: bool) {
        self.oflag(wasi::O_DIRECTORY, directory);
    }

    fn oflag(&mut self, bit: wasi::OFlags, set: bool) {
        if set {
            self.oflags |= bit;
        } else {
            self.oflags &= !bit;
        }
    }

    pub fn append(&mut self, set: bool) {
        self.fdflag(wasi::FDFLAG_APPEND, set);
    }

    pub fn dsync(&mut self, set: bool) {
        self.fdflag(wasi::FDFLAG_DSYNC, set);
    }

    pub fn nonblock(&mut self, set: bool) {
        self.fdflag(wasi::FDFLAG_NONBLOCK, set);
    }

    pub fn rsync(&mut self, set: bool) {
        self.fdflag(wasi::FDFLAG_RSYNC, set);
    }

    pub fn sync(&mut self, set: bool) {
        self.fdflag(wasi::FDFLAG_SYNC, set);
    }

    fn fdflag(&mut self, bit: wasi::FdFlags, set: bool) {
        if set {
            self.fdflags |= bit;
        } else {
            self.fdflags &= !bit;
        }
    }

    pub fn fs_rights_base(&mut self, rights: wasi::Rights) {
        self.rights_base = Some(rights);
    }

    pub fn fs_rights_inheriting(&mut self, rights: wasi::Rights) {
        self.rights_inheriting = Some(rights);
    }

    fn rights_base(&self) -> wasi::Rights {
        if let Some(rights) = self.rights_base {
            return rights;
        }

        // If rights haven't otherwise been specified try to pick a reasonable
        // set. This can always be overridden by users via extension traits, and
        // implementations may give us fewer rights silently than we ask for. So
        // given that, just look at `read` and `write` and bucket permissions
        // based on that.
        let mut base = 0;
        if self.read {
            base |= wasi::RIGHT_FD_READ;
            base |= wasi::RIGHT_FD_READDIR;
        }
        if self.write {
            base |= wasi::RIGHT_FD_WRITE;
            base |= wasi::RIGHT_FD_DATASYNC;
            base |= wasi::RIGHT_FD_ALLOCATE;
            base |= wasi::RIGHT_FD_FILESTAT_SET_SIZE;
        }

        // FIXME: some of these should probably be read-only or write-only...
        base |= wasi::RIGHT_FD_ADVISE;
        base |= wasi::RIGHT_FD_FDSTAT_SET_FLAGS;
        base |= wasi::RIGHT_FD_FILESTAT_SET_TIMES;
        base |= wasi::RIGHT_FD_SEEK;
        base |= wasi::RIGHT_FD_SYNC;
        base |= wasi::RIGHT_FD_TELL;
        base |= wasi::RIGHT_PATH_CREATE_DIRECTORY;
        base |= wasi::RIGHT_PATH_CREATE_FILE;
        base |= wasi::RIGHT_PATH_FILESTAT_GET;
        base |= wasi::RIGHT_PATH_LINK_SOURCE;
        base |= wasi::RIGHT_PATH_LINK_TARGET;
        base |= wasi::RIGHT_PATH_OPEN;
        base |= wasi::RIGHT_PATH_READLINK;
        base |= wasi::RIGHT_PATH_REMOVE_DIRECTORY;
        base |= wasi::RIGHT_PATH_RENAME_SOURCE;
        base |= wasi::RIGHT_PATH_RENAME_TARGET;
        base |= wasi::RIGHT_PATH_SYMLINK;
        base |= wasi::RIGHT_PATH_UNLINK_FILE;
        base |= wasi::RIGHT_POLL_FD_READWRITE;

        return base;
    }

    fn rights_inheriting(&self) -> wasi::Rights {
        self.rights_inheriting.unwrap_or_else(|| self.rights_base())
    }

    pub fn lookup_flags(&mut self, flags: wasi::LookupFlags) {
        self.dirflags = flags;
    }
}

impl File {
    pub fn open(path: &Path, opts: &OpenOptions) -> io::Result<File> {
        let (dir, file) = open_parent(path, wasi::RIGHT_PATH_OPEN)?;
        open_at(&dir, &file, opts)
    }

    pub fn open_at(&self, path: &Path, opts: &OpenOptions) -> io::Result<File> {
        open_at(&self.fd, path, opts)
    }

    pub fn file_attr(&self) -> io::Result<FileAttr> {
        self.fd.filestat_get().map(|meta| FileAttr { meta })
    }

    pub fn metadata_at(
        &self,
        flags: wasi::LookupFlags,
        path: &Path,
    ) -> io::Result<FileAttr> {
        metadata_at(&self.fd, flags, path)
    }

    pub fn fsync(&self) -> io::Result<()> {
        self.fd.sync()
    }

    pub fn datasync(&self) -> io::Result<()> {
        self.fd.datasync()
    }

    pub fn truncate(&self, size: u64) -> io::Result<()> {
        self.fd.filestat_set_size(size)
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_vectored(&mut [IoSliceMut::new(buf)])
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.fd.read(bufs)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        self.write_vectored(&[IoSlice::new(buf)])
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.fd.write(bufs)
    }

    pub fn flush(&self) -> io::Result<()> {
        Ok(())
    }

    pub fn seek(&self, pos: SeekFrom) -> io::Result<u64> {
        self.fd.seek(pos)
    }

    pub fn duplicate(&self) -> io::Result<File> {
        // https://github.com/CraneStation/wasmtime/blob/master/docs/WASI-rationale.md#why-no-dup
        unsupported()
    }

    pub fn set_permissions(&self, _perm: FilePermissions) -> io::Result<()> {
        // Permissions haven't been fully figured out in wasi yet, so this is
        // likely temporary
        unsupported()
    }

    pub fn fd(&self) -> &WasiFd {
        &self.fd
    }

    pub fn into_fd(self) -> WasiFd {
        self.fd
    }

    pub fn read_link(&self, file: &Path) -> io::Result<PathBuf> {
        read_link(&self.fd, file)
    }
}

impl FromInner<u32> for File {
    fn from_inner(fd: u32) -> File {
        unsafe {
            File {
                fd: WasiFd::from_raw(fd),
            }
        }
    }
}

impl DirBuilder {
    pub fn new() -> DirBuilder {
        DirBuilder {}
    }

    pub fn mkdir(&self, p: &Path) -> io::Result<()> {
        let (dir, file) = open_parent(p, wasi::RIGHT_PATH_CREATE_DIRECTORY)?;
        dir.create_directory(file.as_os_str().as_bytes())
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("File")
            .field("fd", &self.fd.as_raw())
            .finish()
    }
}

pub fn readdir(p: &Path) -> io::Result<ReadDir> {
    let mut opts = OpenOptions::new();
    opts.directory(true);
    opts.read(true);
    let dir = File::open(p, &opts)?;
    Ok(ReadDir {
        cookie: Some(0),
        buf: vec![0; 128],
        offset: 0,
        cap: 0,
        inner: Arc::new(ReadDirInner {
            dir,
            root: p.to_path_buf(),
        }),
    })
}

pub fn unlink(p: &Path) -> io::Result<()> {
    let (dir, file) = open_parent(p, wasi::RIGHT_PATH_UNLINK_FILE)?;
    dir.unlink_file(file.as_os_str().as_bytes())
}

pub fn rename(old: &Path, new: &Path) -> io::Result<()> {
    let (old, old_file) = open_parent(old, wasi::RIGHT_PATH_RENAME_SOURCE)?;
    let (new, new_file) = open_parent(new, wasi::RIGHT_PATH_RENAME_TARGET)?;
    old.rename(
        old_file.as_os_str().as_bytes(),
        &new,
        new_file.as_os_str().as_bytes(),
    )
}

pub fn set_perm(_p: &Path, _perm: FilePermissions) -> io::Result<()> {
    // Permissions haven't been fully figured out in wasi yet, so this is
    // likely temporary
    unsupported()
}

pub fn rmdir(p: &Path) -> io::Result<()> {
    let (dir, file) = open_parent(p, wasi::RIGHT_PATH_REMOVE_DIRECTORY)?;
    dir.remove_directory(file.as_os_str().as_bytes())
}

pub fn readlink(p: &Path) -> io::Result<PathBuf> {
    let (dir, file) = open_parent(p, wasi::RIGHT_PATH_READLINK)?;
    read_link(&dir, &file)
}

fn read_link(fd: &WasiFd, file: &Path) -> io::Result<PathBuf> {
    // Try to get a best effort initial capacity for the vector we're going to
    // fill. Note that if it's not a symlink we don't use a file to avoid
    // allocating gigabytes if you read_link a huge movie file by accident.
    // Additionally we add 1 to the initial size so if it doesn't change until
    // when we call `readlink` the returned length will be less than the
    // capacity, guaranteeing that we got all the data.
    let meta = metadata_at(fd, 0, file)?;
    let initial_size = if meta.file_type().is_symlink() {
        (meta.size() as usize).saturating_add(1)
    } else {
        1 // this'll fail in just a moment
    };

    // Now that we have an initial guess of how big to make our buffer, call
    // `readlink` in a loop until it fails or reports it filled fewer bytes than
    // we asked for, indicating we got everything.
    let file = file.as_os_str().as_bytes();
    let mut destination = vec![0u8; initial_size];
    loop {
        let len = fd.readlink(file, &mut destination)?;
        if len < destination.len() {
            destination.truncate(len);
            destination.shrink_to_fit();
            return Ok(PathBuf::from(OsString::from_vec(destination)));
        }
        let amt_to_add = destination.len();
        destination.extend(iter::repeat(0).take(amt_to_add));
    }
}

pub fn symlink(src: &Path, dst: &Path) -> io::Result<()> {
    let (dst, dst_file) = open_parent(dst, wasi::RIGHT_PATH_SYMLINK)?;
    dst.symlink(src.as_os_str().as_bytes(), dst_file.as_os_str().as_bytes())
}

pub fn link(src: &Path, dst: &Path) -> io::Result<()> {
    let (src, src_file) = open_parent(src, wasi::RIGHT_PATH_LINK_SOURCE)?;
    let (dst, dst_file) = open_parent(dst, wasi::RIGHT_PATH_LINK_TARGET)?;
    src.link(
        wasi::LOOKUP_SYMLINK_FOLLOW,
        src_file.as_os_str().as_bytes(),
        &dst,
        dst_file.as_os_str().as_bytes(),
    )
}

pub fn stat(p: &Path) -> io::Result<FileAttr> {
    let (dir, file) = open_parent(p, wasi::RIGHT_PATH_FILESTAT_GET)?;
    metadata_at(&dir, wasi::LOOKUP_SYMLINK_FOLLOW, &file)
}

pub fn lstat(p: &Path) -> io::Result<FileAttr> {
    let (dir, file) = open_parent(p, wasi::RIGHT_PATH_FILESTAT_GET)?;
    metadata_at(&dir, 0, &file)
}

fn metadata_at(
    fd: &WasiFd,
    flags: wasi::LookupFlags,
    path: &Path,
) -> io::Result<FileAttr> {
    fd.path_filestat_get(flags, path.as_os_str().as_bytes())
        .map(|meta| FileAttr { meta })
}

pub fn canonicalize(_p: &Path) -> io::Result<PathBuf> {
    // This seems to not be in wasi's API yet, and we may need to end up
    // emulating it ourselves. For now just return an error.
    unsupported()
}

fn open_at(fd: &WasiFd, path: &Path, opts: &OpenOptions) -> io::Result<File> {
    let fd = fd.open(
        opts.dirflags,
        path.as_os_str().as_bytes(),
        opts.oflags,
        opts.rights_base(),
        opts.rights_inheriting(),
        opts.fdflags,
    )?;
    Ok(File { fd })
}

/// Get pre-opened file descriptors and their paths.
fn get_paths() -> Result<Box<[(PathBuf, WasiFd)]>, wasi::Error> {
    let mut paths = Vec::new();
    for fd in 3.. {
        match wasi::fd_prestat_get(fd) {
            Ok(wasi::Prestat { pr_type: wasi::PREOPENTYPE_DIR, u }) => unsafe {
                let name_len = u.dir.pr_name_len;
                let mut buf = vec![0; name_len];
                wasi::fd_prestat_dir_name(fd, &mut buf)?;
                let path = PathBuf::from(OsString::from_vec(buf));
                paths.push((path, WasiFd::from_raw(fd)));
            },
            Ok(_) => (),
            Err(wasi::EBADF) => break,
            Err(err) => return Err(err),
        }
    }
    Ok(paths.into_boxed_slice())
}

/// Attempts to open a bare path `p`.
///
/// WASI has no fundamental capability to do this. All syscalls and operations
/// are relative to already-open file descriptors. However, we manage a list
/// of preopened file descriptors and their path. In other words, when you want
/// to open a path `p`, you have to find a previously opened file descriptor in
/// this list and then see if `p` is relative to that file descriptor.
///
/// This function, if successful, will return two items:
///
/// * The first is a `ManuallyDrop<WasiFd>`. This represents a preopened file
///   descriptor which we don't have ownership of, but we can use. You shouldn't
///   actually drop the `fd`.
///
/// * The second is a path that should be a part of `p` and represents a
///   relative traversal from the file descriptor specified to the desired
///   location `p`.
///
/// If successful you can use the returned file descriptor to perform
/// file-descriptor-relative operations on the path returned as well. The
/// `rights` argument indicates what operations are desired on the returned file
/// descriptor, and if successful the returned file descriptor should have the
/// appropriate rights for performing `rights` actions.
///
/// Note that this can fail if `p` doesn't look like it can be opened relative
/// to any preopened file descriptor or we have failed to build the list of
/// pre-opened file descriptors.
fn open_parent(
    p: &Path,
    rights: wasi::Rights,
) -> io::Result<(ManuallyDrop<WasiFd>, &Path)> {
    use crate::sync::Once;

    static mut PATHS: Result<Box<[(PathBuf, WasiFd)]>, wasi::Error> = unsafe {
        Err(wasi::Error::new_unchecked(1))
    };
    static PATHS_INIT: Once = Once::new();
    PATHS_INIT.call_once(|| unsafe { PATHS = get_paths(); });

    for (path, fd) in PATHS.as_ref().map_err(|&e| super::err2io(e))?.iter() {
        if let Ok(path) = p.strip_prefix(path) {
            return Ok((ManuallyDrop::new(*fd), path))
        }
    }

    let msg = format!(
        "failed to find a preopened file descriptor \
         through which {:?} could be opened",
        p
    );
    Err(io::Error::new(io::ErrorKind::Other, msg))
}
