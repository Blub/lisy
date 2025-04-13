//! Marker types for namespace types.

use std::ffi::CStr;
use std::io;
use std::marker::PhantomData;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::raw::c_int;

use crate::mount::ns::MountNsInfo;
use crate::open::OpenHow;

/// Marker trait for namespace types. A namespace type has at least an associated procfs name, and
/// a `CLONE_*` constant value.
pub trait Kind {
    /// Name under which the namespace can be found in `/proc/self/ns`.
    const PROCFS_NAME: &'static CStr;

    /// Full "/proc/self/ns/*/` path as c string.
    const PROCFS_PATH: &'static CStr;

    /// The `CLONE_` constant used in `setns(2)`, `clone(2)` or `unshare(2)` for this namespace.
    const TYPE: c_int;
}

macro_rules! define_namespace {
    ($(
        $(#[$doc:meta])+
        ($name:ident, $const:expr, $file:expr, $path:expr)
    ),+ $(,)?) => {
        $(
            $(#[$doc])+
            pub struct $name;

            impl Kind for $name {
                const PROCFS_NAME: &'static ::std::ffi::CStr = $file;
                const PROCFS_PATH: &'static ::std::ffi::CStr = $path;
                const TYPE: ::std::os::raw::c_int = $const;
            }
        )+
    };
}

/// `CLONE_NEWTIME` constant missing from `libc`.
pub const CLONE_NEWTIME: c_int = 0x80;

define_namespace! {
    /// Marker type for a cgroup namespace.
    (CGroup, libc::CLONE_NEWCGROUP, c"cgroup", c"/proc/self/ns/cgroup"),

    /// Marker type for an IPC namespace.
    (Ipc,    libc::CLONE_NEWIPC,    c"ipc",    c"/proc/self/ns/ipc"),

    /// Marker type for a mount namespace.
    (Mnt,    libc::CLONE_NEWNS,     c"mnt",    c"/proc/self/ns/mnt"),

    /// Marker type for a network namespace.
    (Net,    libc::CLONE_NEWNET,    c"net",    c"/proc/self/ns/net"),

    /// Marker type for a PID namespace.
    (Pid,    libc::CLONE_NEWPID,    c"pid",    c"/proc/self/ns/pid"),

    /// Marker type for a time namespace.
    (Time,   libc::CLONE_NEWTIME,   c"time",   c"/proc/self/ns/time"),

    /// Marker type for a user namespace.
    (User,   libc::CLONE_NEWUSER,   c"user",   c"/proc/self/ns/user"),

    /// Marker type for a UTS namespace.
    (Uts,    libc::CLONE_NEWUTS,    c"uts",    c"/proc/self/ns/uts"),
}

/// Marks a namespace type as taking effect immediately on the calling process when using
/// `setns(2)` or `unshare(2)`.
pub trait UnshareDirect {}
/// Marks a namespace type as taking effect only for *child* processes of the calling process after
/// a `setns(2)` or `unshare(2)` call.
pub trait UnshareForChildren {}

impl UnshareDirect for CGroup {}
impl UnshareDirect for Ipc {}
impl UnshareDirect for Mnt {}
impl UnshareDirect for Net {}
impl UnshareDirect for User {}
impl UnshareDirect for Uts {}

impl UnshareForChildren for Time {}

impl UnshareForChildren for Pid {}

/// A typed namespace file descriptor.
pub struct NsFd<K: Kind> {
    fd: OwnedFd,
    _kind: PhantomData<K>,
}

impl<K: Kind> AsRawFd for NsFd<K> {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl<K: Kind> AsFd for NsFd<K> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl<K: Kind> IntoRawFd for NsFd<K> {
    fn into_raw_fd(self) -> RawFd {
        self.fd.into_raw_fd()
    }
}

impl<K: Kind> FromRawFd for NsFd<K> {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
            _kind: PhantomData,
        }
    }
}

impl<K: Kind> NsFd<K> {
    /// Open this process' namespace file descriptor of kind `K`.
    pub fn current() -> io::Result<Self> {
        Ok(Self {
            fd: OpenHow::new_read().open(K::PROCFS_PATH)?,
            _kind: PhantomData,
        })
    }
}

impl NsFd<Mnt> {
    /// Retrieve the mount information for this file descriptor.
    pub fn mount_info(&self) -> io::Result<MountNsInfo> {
        MountNsInfo::get_raw(self.as_raw_fd())
    }

    /// Get the next mount namespace information and a file descriptor for it.
    pub fn next_mount_info(&self) -> io::Result<(MountNsInfo, Self)> {
        MountNsInfo::next_raw(self.as_raw_fd())
    }

    /// Get the previous mount namespace information and a file descriptor for it.
    pub fn previous_mount_info(&self) -> io::Result<(MountNsInfo, Self)> {
        MountNsInfo::previous_raw(self.as_raw_fd())
    }
}
