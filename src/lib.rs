//! # **Li**nux specific **Sy**stem API
//!
//! Higher level APIs targeting newer Linux kernel features.
//!
//! This crate provides somewhat higher level access to more modern features of the Linux kernel, such
//! as builder-style access to the new mount API, `openat2(2)` call with a builder for the `struct
//! open_how` parameters, all the new features of the `statx(2)` system call (such as finding out
//! whether a path is a mount point), or to build user namespace file descriptors (which requires
//! spawning processes and is therefore somewhat inconvenient to do manually).
//!
//! # The new `open` API:
//!
//! In the standard library files are simply opened by path, but this leaves out some important
//! features:
//!
//! - Opening files relative to a directory handle.
//! - Treating such a handle as if we were `chroot`ed into the directory.
//! - Deciding whether or not symlinks should be followed, *not only* for the final component, but also
//!   during the entire path traversal.
//!
//! The `OpenHow` builder can be used for these things.
//!
//! ``` rust, no_run
//! # use std::io;
//! #
//! # trait Context: Sized {
//! #     fn context(self, _: &str) -> Self {
//! #         self
//! #     }
//! # }
//! #
//! # impl<T, E> Context for Result<T, E> {}
//! #
//! # fn code() -> io::Result<()> {
//! #
//! use lisy::open::OpenHow;
//!
//! let ct_dir = OpenHow::new_directory()       // start with `O_DIRECTORY | O_CLOEXEC | O_NOCTTY`
//!     .resolve_no_symlinks(true)              // do not allow *any* symlinks in the path
//!     .resolve_no_xdev(true)                  // do not allow crossing file system boundaries
//!     .open("/my/container")                  // returns an `OwnedFd`
//!     .context("failed to open /my/container")?;
//!
//! // Now open a file within `/my/container` as if it was the root file system:
//! let file_in_container = OpenHow::new_read()
//!     .at_fd(&ct_dir)                         // open relative to `ct_dir`
//!     .resolve_in_root(true)                  // virtually "chroot" into `ct_dir`
//!     .open_file("/usr/share/some/file")      // convenience method to get a `std::fs::File`
//!     .context("failed to open /my/container/usr/share/some/file")?;
//! #
//! # Ok(())
//! # }
//! ```
//!
//! # The new `mount` API:
//!
//! This provides handles representing file systems, superblocks and mount trees.
//!
//! ``` rust, no_run
//! # use std::io;
//! #
//! # trait Context: Sized {
//! #     fn context(self, _: &str) -> Self {
//! #         self
//! #     }
//! # }
//! #
//! # impl<T, E> Context for Result<T, E> {}
//! #
//! # fn code() -> io::Result<()> {
//! #
//! use lisy::mount::{Mount, MountSetAttr, MoveMount, OpenTree};
//! use lisy::userns::{IdMapping, Userns};
//!
//! // Open a directory as a new detached mount tree:
//! let mount = Mount::open_tree("/mnt/a", OpenTree::CLOEXEC | OpenTree::CLONE, 0)
//!     .context("failed to clone tree at /mnt/a")?;
//!
//! // Prepare a user namespace for ID mapping
//! let userns = Userns::builder().context("failed to prepare user namespace")?;
//! userns.map_gids(&[IdMapping::new(0..65536, 100000)])?;
//! userns.map_uids(&[IdMapping::new(0..65536, 100000)])?;
//! let userns = userns
//!     .into_fd()
//!     .context("failed to finish creating user namespace")?;
//!
//! // Apply the namespace
//! mount
//!     .setattr(
//!         &MountSetAttr::new().idmap(&userns),
//!         libc::AT_RECURSIVE | libc::AT_NO_AUTOMOUNT,
//!     )
//!     .context("failed to apply idmapping to mount tree")?;
//!
//! // Bind-mount into place:
//! mount
//!     .move_mount("/mnt/mapped", MoveMount::empty())
//!     .context("failed to move id-mapped mount into place")?;
//! #
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

#[cfg(any(feature = "open", feature = "mount"))]
pub(crate) mod c_path;
#[cfg(any(feature = "open", feature = "mount"))]
use c_path::CPath;

// BEGIN internal helpers
// Getting the feature cfgs right is a bit of a PITA, so it would be nice if there was some tooling
// there?

#[cfg(any(feature = "open", feature = "mount"))]
pub(crate) mod error;

#[cfg(feature = "fs")]
pub(crate) mod bytes;

#[cfg(any(feature = "ns", feature = "pidfd"))]
pub(crate) mod ioctl;

#[cfg(any(feature = "mount", feature = "fs"))]
pub(crate) mod mount_types;

#[cfg(any(feature = "mount", feature = "fs"))]
pub(crate) mod types;

// END internal helpers

#[cfg(feature = "fs")]
pub mod fs;

#[cfg(feature = "mount")]
pub mod mount;

#[cfg(feature = "open")]
pub mod open;

#[cfg(feature = "userns")]
pub mod userns;

#[cfg(feature = "pidfd")]
pub mod pidfd;

#[cfg(feature = "ns")]
pub mod ns;
