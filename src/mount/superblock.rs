//! Superblock instances creates from file systems via `Fs::create`.

use std::convert::TryFrom;
use std::ffi::{CStr, OsStr};
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::raw::{c_int, c_long, c_uint};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use bitflags::bitflags;

use crate::CPath;
use crate::c_path::io_c_string;
use crate::error::io_assert;
use crate::mount::{Mount, sys};

pub use super::sys::MountAttr;

bitflags! {
    /// Mount flags for `Superblock::mount`.
    pub struct FsMount: c_uint {
        /// Set the close-on-exec flag on the file descriptor.
        const CLOEXEC = 0x0000_0001;
    }
}

bitflags! {
    /// Flags for `Superblock::fspick`.
    pub struct FsPick: c_uint {
        /// Set the close-on-exec flag on the file descriptor.
        const CLOEXEC          = 0x0000_0001;

        /// If the final component in the path is a symlink, refuse to follow it.
        const SYMLINK_NOFOLLOW = 0x0000_0002;

        /// Do not trigger auto mounts on this operation.
        const NO_AUTOMOUNT     = 0x0000_0004;

        /// Allow an empty path to pick the file system of the passed file descriptor directly.
        const EMPTY_PATH       = 0x0000_0008;
    }
}

/// Handle to a file system superblock.
///
/// This is a configured, mountable superblock, created either via `Fs::create` to create a new
/// mount point, or by picking an existing mount point via `Superblock::fspick`, which can be used
/// to reconfigure an existing superblock.
pub struct Superblock {
    pub(crate) sb_ref: SuperblockRef,
}

impl AsFd for Superblock {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

impl AsRawFd for Superblock {
    fn as_raw_fd(&self) -> RawFd {
        self.sb_ref.as_raw_fd()
    }
}

impl IntoRawFd for Superblock {
    fn into_raw_fd(self) -> RawFd {
        self.sb_ref.into_raw_fd()
    }
}

impl FromRawFd for Superblock {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        unsafe {
            Self {
                sb_ref: SuperblockRef::from_raw_fd(fd),
            }
        }
    }
}

impl std::ops::Deref for Superblock {
    type Target = SuperblockRef;

    fn deref(&self) -> &Self::Target {
        &self.sb_ref
    }
}

impl std::ops::DerefMut for Superblock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sb_ref
    }
}

impl Superblock {
    /// Pick a file system superblock from a path.
    pub fn fspick<P>(path: &P, fspick: FsPick) -> io::Result<Self>
    where
        P: ?Sized + CPath,
    {
        path.c_path(|path| Self::fspick_at_raw(libc::AT_FDCWD, path, fspick))?
    }

    /// Pick a file system superblock from a path relative to a file descriptor.
    pub fn fspick_at<D, P>(dfd: &D, path: &P, fspick: FsPick) -> io::Result<Self>
    where
        D: ?Sized + AsRawFd,
        P: ?Sized + CPath,
    {
        let dfd = dfd.as_raw_fd();
        path.c_path(|path| Self::fspick_at_raw(dfd, path, fspick))?
    }

    /// Pick a file system superblock from a path relative to a file descriptor.
    pub fn fspick_at_raw(dfd: RawFd, path: &CStr, fspick: FsPick) -> io::Result<Self> {
        let dfd = dfd.as_raw_fd();
        let rc = unsafe { libc::syscall(sys::SYS_fspick, dfd, path.as_ptr(), fspick.bits()) };
        io_assert!(rc >= 0);
        Ok(Self {
            sb_ref: unsafe { SuperblockRef::from_raw_fd(rc as RawFd) },
        })
    }

    /// Pick a file system superblock directly from an already open file descriptor.
    pub fn fspick_fd(dfd: RawFd, fspick: FsPick) -> io::Result<Self> {
        let rc = unsafe {
            libc::syscall(
                sys::SYS_fspick,
                dfd,
                c"",
                fspick.bits() | FsPick::EMPTY_PATH.bits(),
            )
        };
        io_assert!(rc >= 0);
        Ok(Self {
            sb_ref: unsafe { SuperblockRef::from_raw_fd(rc as RawFd) },
        })
    }

    /// Create a detached mount point for the superblock. This mount point can be used with
    /// the `openat` family of functions, and mounted into the file system hierarchy via
    /// `Mount::move`.
    pub fn mount(self, flags: FsMount, mount_attr: MountAttr) -> io::Result<Mount> {
        let rc = unsafe {
            libc::syscall(
                sys::SYS_fsmount,
                self.fd.as_raw_fd(),
                flags.bits(),
                mount_attr.bits(),
            )
        };
        io_assert!(rc >= 0);
        let fd = unsafe { OwnedFd::from_raw_fd(rc as RawFd) };
        Ok(Mount { fd })
    }

    /// Reconfigure an existing superblock.
    pub fn reconfigure(&mut self) -> io::Result<()> {
        let rc = unsafe {
            sys_fsconfig(
                self.fd.as_raw_fd(),
                FsConfig::CmdReconfigure,
                ptr::null(),
                ptr::null(),
                0,
            )
        };
        io_assert!(rc == 0);
        Ok(())
    }
}

#[repr(C)]
pub(crate) enum FsConfig {
    SetFlag = 0,
    SetString = 1,
    SetBinary = 2,
    SetPath = 3,
    SetPathEmpty = 4,
    SetFd = 5,
    CmdCreate = 6,
    CmdReconfigure = 7,
}

pub(crate) unsafe fn sys_fsconfig(
    fd: RawFd,
    cfg: FsConfig,
    key: *const i8,
    value: *const i8,
    aux: c_int,
) -> c_long {
    unsafe { libc::syscall(sys::SYS_fsconfig, fd, cfg, key, value, aux) }
}

/// A reference to a configurable superblock or file system instance.
///
/// This provides the shared methods usable for both by `Fs` and `Superblock` instances.
pub struct SuperblockRef {
    pub(crate) fd: OwnedFd,
}

impl AsFd for SuperblockRef {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

impl AsRawFd for SuperblockRef {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl IntoRawFd for SuperblockRef {
    fn into_raw_fd(self) -> RawFd {
        self.fd.into_raw_fd()
    }
}

impl FromRawFd for SuperblockRef {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        unsafe {
            Self {
                fd: OwnedFd::from_raw_fd(fd),
            }
        }
    }
}

impl SuperblockRef {
    /// Set a flag, such as `noacl` for ext4.
    pub fn set_flag(&self, flag: &str) -> Result<(), io::Error> {
        let flag = io_c_string(flag)?;
        let rc = unsafe {
            sys_fsconfig(
                self.fd.as_raw_fd(),
                FsConfig::SetFlag,
                flag.as_ptr(),
                ptr::null(),
                0,
            )
        };
        io_assert!(rc == 0);
        Ok(())
    }

    /// Set a string value, such as `errors` to `continue` for ext4.
    pub fn set_string<S>(&self, key: &str, value: S) -> Result<(), io::Error>
    where
        S: AsRef<OsStr>,
    {
        let key = io_c_string(key)?;
        let value = io_c_string(value.as_ref().as_bytes())?;
        let rc = unsafe {
            sys_fsconfig(
                self.fd.as_raw_fd(),
                FsConfig::SetString,
                key.as_ptr(),
                value.as_ptr(),
                0,
            )
        };
        io_assert!(rc == 0);
        Ok(())
    }

    /// Set a path option, like the `source` device node to mount.
    pub fn set_path_empty_at<P>(&self, key: &str, value: P, fd: RawFd) -> Result<(), io::Error>
    where
        P: AsRef<Path>,
    {
        let key = io_c_string(key)?;
        let value = io_c_string(value.as_ref().as_os_str().as_bytes())?;
        let rc = unsafe {
            sys_fsconfig(
                self.fd.as_raw_fd(),
                FsConfig::SetPathEmpty,
                key.as_ptr(),
                value.as_ptr(),
                fd,
            )
        };
        io_assert!(rc == 0);
        Ok(())
    }

    /// Set a path option, like the `source` device node to mount. Relative paths are relative to
    /// the file descriptor.
    pub fn set_path_at<P>(&self, key: &str, value: P, fd: RawFd) -> Result<(), io::Error>
    where
        P: AsRef<Path>,
    {
        let key = io_c_string(key)?;
        let value = io_c_string(value.as_ref().as_os_str().as_bytes())?;
        let rc = unsafe {
            sys_fsconfig(
                self.fd.as_raw_fd(),
                FsConfig::SetPath,
                key.as_ptr(),
                value.as_ptr(),
                fd,
            )
        };
        io_assert!(rc == 0);
        Ok(())
    }

    /// Set a path option, like the `source` device node to mount.
    pub fn set_path<P>(&self, key: &str, value: P) -> Result<(), io::Error>
    where
        P: AsRef<Path>,
    {
        self.set_path_at(key, value.as_ref(), libc::AT_FDCWD)
    }

    /// Set a path option, like the `source` device node to mount.
    pub fn set_path_fd(&self, key: &str, fd: RawFd) -> Result<(), io::Error> {
        self.set_path_empty_at(key, Path::new(""), fd)
    }

    /// Set a file descriptor option. This is not meant for paths, use `set_path_fd` for those.
    pub fn set_fd(&self, key: &str, fd: RawFd) -> Result<(), io::Error> {
        let key = io_c_string(key)?;
        let rc = unsafe {
            sys_fsconfig(
                self.fd.as_raw_fd(),
                FsConfig::SetFd,
                key.as_ptr(),
                ptr::null(),
                fd,
            )
        };
        io_assert!(rc == 0);
        Ok(())
    }

    /// Set a binary blob.
    pub fn set_blob(&self, key: &str, blob: &[u8]) -> Result<(), io::Error> {
        let key = io_c_string(key)?;
        let size = c_int::try_from(blob.len()).map_err(io::Error::other)?;
        let rc = unsafe {
            sys_fsconfig(
                self.fd.as_raw_fd(),
                FsConfig::SetBinary,
                key.as_ptr(),
                blob.as_ptr() as _,
                size,
            )
        };
        io_assert!(rc == 0);
        Ok(())
    }
}
