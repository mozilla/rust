//! Unix peer credentials.

// NOTE: Code in this file is heavily based on work done in PR 13 from the tokio-uds repository on
//       GitHub.
//
//       For reference, the link is here: https://github.com/tokio-rs/tokio-uds/pull/13
//       Credit to Martin Habovštiak (GitHub username Kixunil) and contributors for this work.

use libc::{gid_t, pid_t, uid_t};

/// Credentials for a UNIX process for credentials passing.
#[unstable(feature = "peer_credentials_unix_socket", issue = "42839", reason = "unstable")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UCred {
    /// The UID part of the peer credential. This is the effective UID of the process at the domain
    /// socket's endpoint.
    pub uid: uid_t,
    /// The GID part of the peer credential. This is the effective GID of the process at the domain
    /// socket's endpoint.
    pub gid: gid_t,
    /// The PID part of the peer credential. This field is optional because the PID part of the
    /// peer credentials is not supported on every platform. On platforms where the mechanism to
    /// discover the PID exists, this field will be populated to the PID of the process at the
    /// domain socket's endpoint. Otherwise, it will be set to None.
    pub pid: Option<pid_t>,
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub use self::impl_linux::peer_cred;

#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "ios",
    target_os = "macos",
    target_os = "openbsd"
))]
pub use self::impl_bsd::peer_cred;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod impl_linux {
    use super::UCred;
    use crate::os::unix::io::AsRawFd;
    use crate::os::unix::net::UnixStream;
    use crate::{io, mem};

    pub fn peer_cred(socket: &UnixStream) -> io::Result<UCred> {
        use libc::{c_void, ucred};

        let ucred_size = mem::size_of::<ucred>();

        // Trivial sanity checks.
        assert!(mem::size_of::<u32>() <= mem::size_of::<usize>());
        assert!(ucred_size <= u32::MAX as usize);

        let mut ucred_size = ucred_size as u32;
        let mut ucred: ucred = ucred { pid: 1, uid: 1, gid: 1 };

        unsafe {
            let ret = libc::getsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut ucred as *mut ucred as *mut c_void,
                &mut ucred_size,
            );

            if ret == 0 && ucred_size as usize == mem::size_of::<ucred>() {
                Ok(UCred { uid: ucred.uid, gid: ucred.gid, pid: Some(ucred.pid) })
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[cfg(any(
    target_os = "dragonfly",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd"
))]
pub mod impl_bsd {
    use super::UCred;
    use crate::io;
    use crate::os::unix::io::AsRawFd;
    use crate::os::unix::net::UnixStream;

    pub fn peer_cred(socket: &UnixStream) -> io::Result<UCred> {
        let mut cred = UCred { uid: 1, gid: 1, pid: None };
        unsafe {
            let ret = libc::getpeereid(socket.as_raw_fd(), &mut cred.uid, &mut cred.gid);

            if ret == 0 { Ok(cred) } else { Err(io::Error::last_os_error()) }
        }
    }
}
