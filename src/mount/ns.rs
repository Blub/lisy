//! Mount namespace related code.

use std::io;
use std::mem::size_of;
use std::os::fd::{AsRawFd, RawFd};
use std::os::raw::c_int;

use crate::error::io_assert;

/// Information about a mount namespace.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MountNsInfo {
    size: u32,
    /// Number of mounts in the namespace.
    pub nr_mounts: u32,
    /// The namespace's ID.
    pub mnt_ns_id: u64,
}

const NS_MNT_GET_INFO: c_int = crate::ioctl::ior::<MountNsInfo>(0xb7, 10);

impl MountNsInfo {
    /// Retrieve the mount information for a file descriptor.
    pub fn get<F: ?Sized + AsRawFd>(fd: &F) -> io::Result<Self> {
        Self::get_raw(fd.as_raw_fd())
    }

    /// Retrieve the mount information for a raw file descriptor.
    pub fn get_raw(fd: RawFd) -> io::Result<Self> {
        let mut info = Self {
            size: u32::try_from(size_of::<Self>()).unwrap(),
            nr_mounts: 0,
            mnt_ns_id: 0,
        };

        let rc = unsafe { libc::ioctl(fd, NS_MNT_GET_INFO as _, &mut info) };
        io_assert!(rc == 0);

        Ok(info)
    }
}
