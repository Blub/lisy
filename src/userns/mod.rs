//! Module for simplified user namespace creation. Requires kernel >=5.3.

use std::error::Error as StdError;
use std::ffi::{c_int, c_void};
use std::fmt;
use std::io;
use std::ops::Range;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};

use crate::open::OpenHow;

mod pipe;
use pipe::Pipe;

/// A handle to a user namespace.
pub struct Userns {
    fd: OwnedFd,
}

extern "C" fn userns_process(pipe: *mut c_void) -> c_int {
    let pipe = unsafe { Box::from_raw(pipe as *mut Pipe) };
    let Pipe { readable, writable } = *pipe;

    drop(writable);
    let mut scratch = [0i8; 8];
    let _ = unsafe {
        libc::read(
            readable.as_raw_fd(),
            scratch.as_mut_ptr() as *mut libc::c_void,
            8,
        )
    };

    0
}

impl Userns {
    unsafe fn from_raw(fd: OwnedFd) -> Self {
        Self { fd }
    }

    /// Create a builder for a user namespace.
    ///
    /// This spawns a process in the background.
    pub fn builder() -> io::Result<UsernsBuilder> {
        let mut pid_fd: c_int = -1;

        // Used to terminate the child process...
        // to deal with glibc's clone implementation we put these on the heap:
        let mut pipe = Box::new(Pipe::new()?);

        let pid = unsafe {
            const STACK_SIZE: usize = 64 * 1024;
            let stack = std::alloc::alloc(std::alloc::Layout::array::<u8>(STACK_SIZE).unwrap());
            let stack = Box::from_raw(std::ptr::slice_from_raw_parts_mut(stack, STACK_SIZE));
            libc::clone(
                userns_process,
                stack.as_ptr().add(STACK_SIZE) as *mut c_void,
                libc::CLONE_NEWUSER | libc::CLONE_PIDFD | libc::SIGCHLD,
                &mut *pipe as *mut Pipe as *mut c_void,
                &mut pid_fd as *mut c_int as *mut libc::pid_t,
            )
        };

        if pid < 0 {
            return Err(io::Error::last_os_error());
        }
        let pid_fd = unsafe { OwnedFd::from_raw_fd(pid_fd) };

        let Pipe { readable, writable } = *pipe;
        drop(readable);

        let uid_map = OpenHow::new_write().open(&format!("/proc/{pid}/uid_map"))?;
        let gid_map = OpenHow::new_write().open(&format!("/proc/{pid}/gid_map"))?;

        drop(writable);

        Ok(UsernsBuilder {
            pid: Some(pid),
            pid_fd,
            uid_map: Some(uid_map),
            gid_map: Some(gid_map),
        })
    }
}

impl AsRawFd for Userns {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl IntoRawFd for Userns {
    fn into_raw_fd(self) -> RawFd {
        self.fd.into_raw_fd()
    }
}

impl FromRawFd for Userns {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        unsafe { Self::from_raw(OwnedFd::from_raw_fd(fd)) }
    }
}

impl AsFd for Userns {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

/// A builder for a user namespace.
pub struct UsernsBuilder {
    // must not removed except in the drop handler
    pid: Option<libc::pid_t>,
    pid_fd: OwnedFd,
    uid_map: Option<OwnedFd>,
    gid_map: Option<OwnedFd>,
}

impl Drop for UsernsBuilder {
    fn drop(&mut self) {
        if let Some(pid) = self.pid.take() {
            let _ = kill_process(&self.pid_fd, pid);
        }
    }
}

fn kill_process(pid_fd: &OwnedFd, pid: libc::pid_t) -> io::Result<()> {
    unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pid_fd.as_raw_fd(),
            libc::SIGKILL,
            std::ptr::null::<c_void>(),
            0,
        )
    };
    loop {
        let rc = unsafe { libc::waitpid(pid, std::ptr::null_mut(), 0) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        if rc == pid {
            break;
        }
    }
    Ok(())
}

/// Maps a range of ids from/to a namespace.
#[derive(Clone, Copy, Debug)]
pub struct IdMapping {
    /// The user id inside the name space.
    pub ns_id: u32,

    /// The user id inside the parent namespace.
    pub parent_id: u32,

    /// The number of consecutive user ids.
    pub len: u32,
}

impl IdMapping {
    /// Create a new ID mapping, mapping a range of parent namespace IDs to a new range in the user
    /// namespace.
    pub fn new(range: Range<u32>, to: u32) -> Self {
        Self {
            ns_id: range.start,
            parent_id: to,
            len: range.end - range.start,
        }
    }

    /// Parse the common format of `<ns id>:<host id>:<count>`.
    pub fn parse_common(s: &str) -> Result<Self, ParseIdMappingError> {
        let mut parts = s.splitn(3, ':');
        Ok(Self {
            ns_id: parts
                .next()
                .ok_or(ParseIdMappingError)?
                .parse::<u32>()
                .map_err(|_| ParseIdMappingError)?,
            parent_id: parts
                .next()
                .ok_or(ParseIdMappingError)?
                .parse::<u32>()
                .map_err(|_| ParseIdMappingError)?,
            len: parts
                .next()
                .ok_or(ParseIdMappingError)?
                .parse::<u32>()
                .map_err(|_| ParseIdMappingError)?,
        })
    }
}

impl From<(u32, u32, u32)> for IdMapping {
    fn from((ns_id, parent_id, len): (u32, u32, u32)) -> Self {
        Self {
            ns_id,
            parent_id,
            len,
        }
    }
}

impl UsernsBuilder {
    /// Setup the user id mapping in the namespace, this can only be called once.
    pub fn map_uids(&self, mapping: &[IdMapping]) -> io::Result<()> {
        // unwrap: we only remove these in the "into_fd" function.
        Self::map_do(self.uid_map.as_ref().unwrap(), mapping)
    }

    /// Setup the group id mapping in the namespace, this can only be called once.
    pub fn map_gids(&self, mapping: &[IdMapping]) -> io::Result<()> {
        // unwrap: we only remove these in the "into_fd" function.
        Self::map_do(self.gid_map.as_ref().unwrap(), mapping)
    }

    fn map_do(fd: &OwnedFd, mapping: &[IdMapping]) -> io::Result<()> {
        use std::io::Write as _;

        let mut data = Vec::new();
        for entry in mapping {
            writeln!(data, "{} {} {}", entry.ns_id, entry.parent_id, entry.len)?;
        }

        let rc = unsafe {
            libc::write(
                fd.as_raw_fd(),
                data.as_ptr() as *const libc::c_void,
                data.len(),
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }

    /// Open the namespace file descriptor and drop the reference to the underlying helper process.
    pub fn into_fd(mut self) -> io::Result<Userns> {
        let pid = self.pid.unwrap(); // we only take this out in the drop handler
        let fd = OpenHow::new_read().open(&format!("/proc/{pid}/ns/user"))?;
        // close the file descriptors
        self.uid_map = None;
        self.gid_map = None;
        loop {
            let rc = unsafe { libc::waitpid(pid, std::ptr::null_mut(), 0) };
            if rc < 0 {
                return Err(io::Error::last_os_error());
            }
            if rc == pid {
                break;
            }
        }
        self.pid = None; // disarm the drop handler
        Ok(unsafe { Userns::from_raw(fd) })
    }
}

/// An error parsing a user/group id mapping.
#[derive(Debug)]
pub struct ParseIdMappingError;

impl fmt::Display for ParseIdMappingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("failed to parse id mapping")
    }
}

impl StdError for ParseIdMappingError {}
