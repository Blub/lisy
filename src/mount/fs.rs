//! File system instances.
//!
//! Most of the time, the first step in mounting a file system is by getting a handle to the file
//! system type. See the `Fs` documentation for details.

use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::raw::c_uint;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::ptr;

use bitflags::bitflags;

use crate::c_path::io_c_string;
use crate::error::io_assert;
use crate::mount::superblock::{FsConfig, SuperblockRef, sys_fsconfig};
use crate::mount::{Superblock, sys};

bitflags! {
    /// Flags for `Fs::open`.
    pub struct FsOpen: c_uint {
        /// Set the close-on-exec flag on the file descriptor.
        const CLOEXEC = 0x0000_0001;
    }
}

/// Represents a handle to a file system.
///
/// This is the first step to mounting a new file system. First, a file system handle is acquired,
/// by passing the file system name (the `-t` option in a `mount` command), to `open`.
/// The resulting handle can then be configured with options. This is anything otherwise passed via
/// the `-o` option in a `mount` command.
///
/// After configuring, the `Superblock` can be instantiated via the `create` method.
pub struct Fs {
    sb_ref: SuperblockRef,
}

impl AsFd for Fs {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

impl AsRawFd for Fs {
    fn as_raw_fd(&self) -> RawFd {
        self.sb_ref.as_raw_fd()
    }
}

impl IntoRawFd for Fs {
    fn into_raw_fd(self) -> RawFd {
        self.sb_ref.into_raw_fd()
    }
}

impl FromRawFd for Fs {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        unsafe {
            Self {
                sb_ref: SuperblockRef::from_raw_fd(fd),
            }
        }
    }
}

impl std::ops::Deref for Fs {
    type Target = SuperblockRef;

    fn deref(&self) -> &Self::Target {
        &self.sb_ref
    }
}

impl std::ops::DerefMut for Fs {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sb_ref
    }
}

impl Fs {
    /// Open a file system driver, such as `"xfs"`.
    pub fn open(fs_type: &str, flags: FsOpen) -> io::Result<Self> {
        let fs_type = io_c_string(fs_type)?;

        let rc = unsafe { libc::syscall(sys::SYS_fsopen, fs_type.as_ptr(), flags.bits()) };
        io_assert!(rc >= 0);

        Ok(Self {
            sb_ref: unsafe { SuperblockRef::from_raw_fd(rc as RawFd) },
        })
    }

    /// Create a [`Superblock`] with the current configuration.
    pub fn create(self) -> io::Result<Superblock> {
        let rc = unsafe {
            sys_fsconfig(
                self.fd.as_raw_fd(),
                FsConfig::CmdCreate,
                ptr::null(),
                ptr::null(),
                0,
            )
        };
        io_assert!(rc == 0);
        Ok(Superblock {
            sb_ref: self.sb_ref,
        })
    }
}
