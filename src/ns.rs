//! Marker types for namespace types.

use std::ffi::CStr;
use std::os::raw::c_int;

/// Marker trait for namespace types. A namespace type has at least an associated procfs name, and
/// a `CLONE_*` constant value.
pub trait Kind {
    /// Name under which the namespace can be found in `/proc/self/ns`.
    const PROCFS_NAME: &'static CStr;

    /// The `CLONE_` constant used in `setns(2)`, `clone(2)` or `unshare(2)` for this namespace.
    const TYPE: c_int;
}

macro_rules! define_namespace {
    ($(
        $(#[$doc:meta])+
        ($name:ident, $const:expr, $file:expr)
    ),+ $(,)?) => {
        $(
            $(#[$doc])+
            pub struct $name;

            impl Kind for $name {
                const PROCFS_NAME: &'static ::std::ffi::CStr = $file;
                const TYPE: ::std::os::raw::c_int = $const;
            }
        )+
    };
}

/// `CLONE_NEWTIME` constant missing from `libc`.
pub const CLONE_NEWTIME: c_int = 0x80;

define_namespace! {
    /// Marker type for a cgroup namespace.
    (CGroup, libc::CLONE_NEWCGROUP, c"cgroup"),

    /// Marker type for an IPC namespace.
    (Ipc,    libc::CLONE_NEWIPC,    c"ipc"),

    /// Marker type for a mount namespace.
    (Mnt,    libc::CLONE_NEWNS,     c"mnt"),

    /// Marker type for a network namespace.
    (Net,    libc::CLONE_NEWNET,    c"net"),

    /// Marker type for a PID namespace.
    (Pid,    libc::CLONE_NEWPID,    c"pid"),

    /// Marker type for a time namespace.
    (Time,   libc::CLONE_NEWTIME,   c"time"),

    /// Marker type for a user namespace.
    (User,   libc::CLONE_NEWUSER,   c"user"),

    /// Marker type for a UTS namespace.
    (Uts,    libc::CLONE_NEWUTS,    c"uts"),
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
