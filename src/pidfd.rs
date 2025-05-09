//! `pidfds` are handles to processes which can be polled and used to send signals and other
//! operations, they are much more powerful than numerical PIDs.

use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};

use crate::error::io_assert;
use crate::ns::{Mnt, NsFd};

#[rustfmt::skip]
mod ioctls {
    use std::ffi::c_int;

    use crate::ioctl::io;

    pub const PIDFS_IOCTL_MAGIC: c_int = 0xFF;

    // pub const PIDFD_GET_CGROUP_NAMESPACE            : c_int = io(PIDFS_IOCTL_MAGIC, 1);
    // pub const PIDFD_GET_IPC_NAMESPACE               : c_int = io(PIDFS_IOCTL_MAGIC, 2);
    pub const PIDFD_GET_MNT_NAMESPACE               : c_int = io(PIDFS_IOCTL_MAGIC, 3);
    // pub const PIDFD_GET_NET_NAMESPACE               : c_int = io(PIDFS_IOCTL_MAGIC, 4);
    // pub const PIDFD_GET_PID_NAMESPACE               : c_int = io(PIDFS_IOCTL_MAGIC, 5);
    // pub const PIDFD_GET_PID_FOR_CHILDREN_NAMESPACE  : c_int = io(PIDFS_IOCTL_MAGIC, 6);
    // pub const PIDFD_GET_TIME_NAMESPACE              : c_int = io(PIDFS_IOCTL_MAGIC, 7);
    // pub const PIDFD_GET_TIME_FOR_CHILDREN_NAMESPACE : c_int = io(PIDFS_IOCTL_MAGIC, 8);
    // pub const PIDFD_GET_USER_NAMESPACE              : c_int = io(PIDFS_IOCTL_MAGIC, 9);
    // pub const PIDFD_GET_UTS_NAMESPACE               : c_int = io(PIDFS_IOCTL_MAGIC, 10);
}

/// A pid file descriptor is a handle to a process.
///
/// Contrary to numerical pids, pidfds cannot be reused while a handle exists. Signals can be sent
/// to the process and they enable waiting for processes via polling mechanisms such as `epoll`.
#[derive(Debug)]
pub struct PidFd {
    fd: OwnedFd,
}

impl AsRawFd for PidFd {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl IntoRawFd for PidFd {
    fn into_raw_fd(self) -> RawFd {
        self.fd.into_raw_fd()
    }
}

impl FromRawFd for PidFd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
        }
    }
}

impl AsFd for PidFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl PidFd {
    /// Get a pid fd to the current process.
    pub fn this() -> io::Result<Self> {
        unsafe {
            let pid = libc::getpid();
            let fd = libc::syscall(libc::SYS_pidfd_open, pid, 0);
            io_assert!(fd >= 0);
            Ok(Self::from_raw_fd(i32::try_from(fd).unwrap()))
        }
    }

    /// Get a handle to the process' mount namespace.
    pub fn mount_namespace(&self) -> io::Result<NsFd<Mnt>> {
        unsafe {
            let fd = libc::ioctl(self.as_raw_fd(), ioctls::PIDFD_GET_MNT_NAMESPACE as u64, 0);
            io_assert!(fd >= 0);
            Ok(NsFd::from_raw_fd(fd))
        }
    }
}
