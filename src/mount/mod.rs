//! Linux >=5.2 file system mount API.
//!
//! This crate provides access to the kernel's new mount API.

use std::ffi::{CStr, c_int};
use std::io;

use crate::CPath;
use crate::error::io_assert;

pub mod sys;

pub mod fs;
#[doc(inline)]
pub use fs::{Fs, FsOpen};

pub mod superblock;
#[doc(inline)]
pub use superblock::{FsMount, FsPick, MountAttr, Superblock};

pub mod mount;
#[doc(inline)]
pub use mount::{Mount, MountSetAttr, MoveMount, OpenTree};

/// Wrapper for the `umount2(2)` system call.
pub fn umount<P>(path: &P, flags: c_int) -> io::Result<()>
where
    P: ?Sized + CPath,
{
    fn umount_do(path: &CStr, flags: c_int) -> io::Result<()> {
        let rc = unsafe { libc::umount2(path.as_ptr(), flags) };
        io_assert!(rc == 0);
        Ok(())
    }

    path.c_path(|path| umount_do(path, flags))?
}
