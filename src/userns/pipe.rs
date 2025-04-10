//! Simple pipe fd helper

use std::io;
use std::os::fd::{FromRawFd, OwnedFd};

/// A `pipe(2)` helper.
pub struct Pipe {
    pub readable: OwnedFd,
    pub writable: OwnedFd,
}

impl Pipe {
    /// Create a new `pipe(2)`.
    pub fn new() -> io::Result<Self> {
        let mut fds = [-1i32; 2];

        let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(unsafe {
            Self {
                readable: OwnedFd::from_raw_fd(fds[0]),
                writable: OwnedFd::from_raw_fd(fds[1]),
            }
        })
    }
}
