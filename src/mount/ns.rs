//! Mount namespace related code.

use std::io;
use std::mem::size_of;
use std::os::fd::{AsRawFd, RawFd};
use std::os::raw::c_int;

#[cfg(feature = "ns")]
use std::os::fd::FromRawFd;

use crate::error::io_assert;
#[cfg(feature = "ns")]
use crate::ns::{Mnt, NsFd};

use crate::mount_types::MountNsId;

/// Information about a mount namespace.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MountNsInfo {
    size: u32,
    /// Number of mounts in the namespace.
    pub nr_mounts: u32,
    /// The namespace's ID.
    pub mnt_ns_id: MountNsId,
}

const NS_MNT_GET_INFO: c_int = crate::ioctl::ior::<MountNsInfo>(0xb7, 10);
const NS_MNT_GET_NEXT: c_int = crate::ioctl::ior::<MountNsInfo>(0xb7, 11);
const NS_MNT_GET_PREV: c_int = crate::ioctl::ior::<MountNsInfo>(0xb7, 12);

impl MountNsInfo {
    fn info_ioctl(fd: RawFd, ctl: c_int) -> io::Result<(Self, c_int)> {
        let mut info = Self {
            size: u32::try_from(size_of::<Self>()).unwrap(),
            nr_mounts: 0,
            mnt_ns_id: MountNsId::from_raw(0),
        };

        let rc = unsafe { libc::ioctl(fd, ctl as _, &mut info) };

        Ok((info, rc))
    }

    /// Retrieve the mount information for a file descriptor.
    pub fn get<F: ?Sized + AsRawFd>(fd: &F) -> io::Result<Self> {
        Self::get_raw(fd.as_raw_fd())
    }

    /// Retrieve the mount information for a raw file descriptor.
    pub fn get_raw(fd: RawFd) -> io::Result<Self> {
        let (info, rc) = Self::info_ioctl(fd, NS_MNT_GET_INFO)?;
        io_assert!(rc == 0);
        Ok(info)
    }

    #[cfg(feature = "ns")]
    /// Get the next mount namespace information and a file descriptor for it.
    pub fn next<F: ?Sized + AsRawFd>(fd: &F) -> io::Result<(Self, NsFd<Mnt>)> {
        Self::next_raw(fd.as_raw_fd())
    }

    #[cfg(feature = "ns")]
    /// Get the next mount namespace information and a file descriptor for it.
    pub fn next_raw(fd: RawFd) -> io::Result<(Self, NsFd<Mnt>)> {
        let (info, rc) = Self::info_ioctl(fd, NS_MNT_GET_NEXT)?;
        io_assert!(rc >= 0);
        let fd = unsafe { NsFd::from_raw_fd(rc) };
        Ok((info, fd))
    }

    #[cfg(feature = "ns")]
    /// Get the previous mount namespace information and a file descriptor for it.
    pub fn previous<F: ?Sized + AsRawFd>(fd: &F) -> io::Result<(Self, NsFd<Mnt>)> {
        Self::previous_raw(fd.as_raw_fd())
    }

    #[cfg(feature = "ns")]
    /// Get the previous mount namespace information and a file descriptor for it.
    pub fn previous_raw(fd: RawFd) -> io::Result<(Self, NsFd<Mnt>)> {
        let (info, rc) = Self::info_ioctl(fd, NS_MNT_GET_PREV)?;
        io_assert!(rc >= 0);
        let fd = unsafe { NsFd::from_raw_fd(rc) };
        Ok((info, fd))
    }
}
