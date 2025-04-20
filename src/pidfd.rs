//! `pidfds` are handles to processes which can be polled and used to send signals and other
//! operations, they are much more powerful than numerical PIDs.

use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};

use crate::error::io_assert;
use crate::ns::NsFd;

#[rustfmt::skip]
mod ioctls {
    use std::ffi::c_int;

    use crate::ioctl::{io, iowr};

    use super::CPidFdInfo;

    pub const PIDFS_IOCTL_MAGIC: c_int = 0xFF;

    pub const PIDFD_GET_CGROUP_NAMESPACE            : c_int = io(PIDFS_IOCTL_MAGIC, 1);
    pub const PIDFD_GET_IPC_NAMESPACE               : c_int = io(PIDFS_IOCTL_MAGIC, 2);
    pub const PIDFD_GET_MNT_NAMESPACE               : c_int = io(PIDFS_IOCTL_MAGIC, 3);
    pub const PIDFD_GET_NET_NAMESPACE               : c_int = io(PIDFS_IOCTL_MAGIC, 4);
    pub const PIDFD_GET_PID_NAMESPACE               : c_int = io(PIDFS_IOCTL_MAGIC, 5);
    pub const PIDFD_GET_PID_FOR_CHILDREN_NAMESPACE  : c_int = io(PIDFS_IOCTL_MAGIC, 6);
    pub const PIDFD_GET_TIME_NAMESPACE              : c_int = io(PIDFS_IOCTL_MAGIC, 7);
    pub const PIDFD_GET_TIME_FOR_CHILDREN_NAMESPACE : c_int = io(PIDFS_IOCTL_MAGIC, 8);
    pub const PIDFD_GET_USER_NAMESPACE              : c_int = io(PIDFS_IOCTL_MAGIC, 9);
    pub const PIDFD_GET_UTS_NAMESPACE               : c_int = io(PIDFS_IOCTL_MAGIC, 10);
    pub const PIDFD_GET_INFO                        : c_int = iowr::<CPidFdInfo>(PIDFS_IOCTL_MAGIC, 11);
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

macro_rules! ns_fd_getters {
    (
        $(
            $(#[$doc:meta])+
            $name:ident($ioctl:expr) -> $nsty:ident;
        )+
    ) => {
        $(
            $(#[$doc])+
            pub fn $name(&self) -> io::Result<NsFd<crate::ns::$nsty>> {
                unsafe {
                    let fd = libc::ioctl(self.as_raw_fd(), $ioctl as u64, 0);
                    io_assert!(fd >= 0);
                    Ok(NsFd::from_raw_fd(fd))
                }
            }
        )+
    };
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

    ns_fd_getters! {
        /// Get a handle to the process' cgroup namespace.
        cgroup_namespace(ioctls::PIDFD_GET_CGROUP_NAMESPACE) -> CGroup;

        /// Get a handle to the process' IPC namespace.
        ipc_namespace(ioctls::PIDFD_GET_IPC_NAMESPACE) -> Ipc;

        /// Get a handle to the process' mount namespace.
        mount_namespace(ioctls::PIDFD_GET_MNT_NAMESPACE) -> Mnt;

        /// Get a handle to the process' network namespace.
        network_namespace(ioctls::PIDFD_GET_NET_NAMESPACE) -> Net;

        /// Get a handle to the process' *own* PID namespace.
        ///
        /// This is the PID namespace the process lives under, as opposed to the PID namespace new
        /// subprocesses will be spawned under, which can be queried via
        /// [`pid_namespace_for_children`](PidFd::pid_namespace_for_children()).
        pid_namespace(ioctls::PIDFD_GET_PID_NAMESPACE) -> Pid;

        /// Get a handle to the PID namespace this process' children will be spawned in.
        ///
        /// This is the namespace for child processes after the process has used `setns()` or
        /// `unshare()` for the PID namespace, as pid namespaces cannot be entered directly.
        ///
        /// To get the process' *own* PID namespace, use [`pid_namespace`](PidFd::pid_namespace()).
        pid_namespace_for_children(ioctls::PIDFD_GET_PID_FOR_CHILDREN_NAMESPACE) -> Pid;

        /// Get a handle to the process' *own* time namespace.
        ///
        /// This is the time namespace the process lives under, as opposed to the time namespace
        /// new subprocesses will be spawned under, which can be queried via
        /// [`time_namespace_for_children`](PidFd::time_namespace_for_children()).
        time_namespace(ioctls::PIDFD_GET_TIME_NAMESPACE) -> Time;

        /// Get a handle to the time namespace this process' children will be spawned in.
        ///
        /// This is the namespace for child processes after the process has used
        /// `unshare(CLONE_NEWTIME)`, as this does not immediately enter a time namespace.
        ///
        /// To get the process' *own* time namespace, use [`time_namespace`](PidFd::time_namespace()).
        time_namespace_for_children(ioctls::PIDFD_GET_TIME_FOR_CHILDREN_NAMESPACE) -> Time;

        /// Get a handle to the process' user namespace.
        user_namespace(ioctls::PIDFD_GET_USER_NAMESPACE) -> User;

        /// Get a handle to the process' UTS namespace.
        uts_namespace(ioctls::PIDFD_GET_UTS_NAMESPACE) -> Uts;
    }

    /// Query information about the process.
    ///
    /// While the kernel provides bitflags for which information to query, all the current ones are
    /// returned even if not requested. For maximum compatibility, these flags should be included
    /// in the request, so this should be called as `fd.info(Default::default())`.
    pub fn info(&self, flags: GetInfoFlags) -> io::Result<Info> {
        unsafe {
            let mut info = Info {
                raw: CPidFdInfo {
                    mask: flags.bits(),
                    ..std::mem::zeroed()
                },
            };
            let rc = libc::ioctl(
                self.as_raw_fd(),
                ioctls::PIDFD_GET_INFO as u64,
                &raw mut info.raw,
            );
            io_assert!(rc == 0);
            Ok(info)
        }
    }
}

bitflags::bitflags! {
    /// Mount attributes for `Superblock::mount` or Mount::setattr.
    ///
    /// The default value is `PID | CREDS | CGROUP_ID` as these are returned even if not
    /// requested.
    #[derive(Clone, Copy, Debug)]
    #[repr(transparent)]
    pub struct GetInfoFlags: u64 {
        /// Get the PID. Always returned, even if not requested.
        const PID       = 0x0000_0001;

        /// Get the credential information. Always returned, even if not requested.
        const CREDS     = 0x0000_0002;

        /// Get the cgroup id. Always returned, even if not requested.
        const CGROUP_ID = 0x0000_0004;

        /// Get the exit code (if the process has exited).
        const EXIT      = 0x0000_0008;
    }
}

impl Default for GetInfoFlags {
    fn default() -> Self {
        const { Self::from_bits(Self::PID.bits() | Self::CREDS.bits() | Self::CGROUP_ID.bits()).unwrap() }
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
struct CPidFdInfo {
    mask: u64,
    cgroupid: u64,
    pid: u32,
    tgid: u32,
    ppid: u32,
    ruid: u32,
    rgid: u32,
    euid: u32,
    egid: u32,
    suid: u32,
    sgid: u32,
    fsuid: u32,
    fsgid: u32,
    exit_code: i32,
}

/// Information about a process retrieved via [`PidFd::info`](PidFd::info()).
#[derive(Clone, Debug)]
pub struct Info {
    raw: CPidFdInfo,
}

impl Info {
    fn maybe<T>(&self, flags: GetInfoFlags, value: T) -> Option<T> {
        (self.raw.mask & flags.bits() == flags.bits()).then_some(value)
    }

    /// Get the PID.
    pub fn pid(&self) -> Option<libc::pid_t> {
        self.maybe(GetInfoFlags::PID, self.raw.pid as libc::pid_t)
    }

    /// Get the cgroup ID
    pub fn cgroup_id(&self) -> Option<u64> {
        self.maybe(GetInfoFlags::CGROUP_ID, self.raw.cgroupid)
    }

    /// Get the thread group id.
    pub fn thread_group_id(&self) -> Option<u32> {
        self.maybe(GetInfoFlags::PID, self.raw.tgid)
    }

    /// Get the parent PID.
    pub fn parent_pid(&self) -> Option<libc::pid_t> {
        self.maybe(GetInfoFlags::PID, self.raw.ppid as libc::pid_t)
    }

    /// Get the process' exit code if it has exited.
    pub fn exit_code(&self) -> Option<i32> {
        self.maybe(GetInfoFlags::EXIT, self.raw.exit_code)
    }

    /// Get the credentials.
    pub fn credentials(&self) -> Option<Credentials> {
        self.maybe(
            GetInfoFlags::CREDS,
            Credentials {
                ruid: self.raw.ruid as libc::uid_t,
                rgid: self.raw.rgid as libc::gid_t,
                euid: self.raw.euid as libc::uid_t,
                egid: self.raw.egid as libc::gid_t,
                suid: self.raw.suid as libc::uid_t,
                sgid: self.raw.sgid as libc::gid_t,
                fsuid: self.raw.fsuid as libc::uid_t,
                fsgid: self.raw.fsgid as libc::gid_t,
            },
        )
    }
}

/// Credentials of a process.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Credentials {
    /// The real user id.
    pub ruid: libc::uid_t,

    /// The real group id.
    pub rgid: libc::gid_t,

    /// The effective user id.
    pub euid: libc::uid_t,

    /// The effective group id.
    pub egid: libc::gid_t,

    /// The saved user id.
    pub suid: libc::uid_t,

    /// The saved group id.
    pub sgid: libc::gid_t,

    /// The file system user id.
    pub fsuid: libc::uid_t,

    /// The file system group id.
    pub fsgid: libc::gid_t,
}
