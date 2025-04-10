use std::ffi::CStr;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::path::Component;
use std::path::Path;

use crate::error::{io_bail, io_format_err};
use crate::open::OpenHow;

/// Builder style helper to decide how to create a path with all its subdirectories.
///
/// This "chases" directories via `openat()` and file descriptors to enable functionality such as
/// disallowing the resolution of symlinks or creating a path within a directory as if it was the
/// current "root" directory.
#[derive(Clone, Debug)]
pub struct CreatePath {
    mode: libc::mode_t,
    allow_symlinks: bool,
    resolve_in_root: bool,
}

impl Default for CreatePath {
    fn default() -> Self {
        Self::new()
    }
}

impl CreatePath {
    /// By default, the mode is set to `0o777` (the umask will apply), symlink traversal is
    /// allowed.
    pub const fn new() -> Self {
        Self {
            mode: 0o777,
            allow_symlinks: false,
            resolve_in_root: false,
        }
    }

    /// Change the mode of the directories to be set.
    pub const fn mode(mut self, mode: libc::mode_t) -> Self {
        self.mode = mode;
        self
    }

    /// Set whether or not symlinks should be followed for the already existing path elements.
    pub const fn allow_symlinks(mut self, allow_symlinks: bool) -> Self {
        self.allow_symlinks = allow_symlinks;
        self
    }

    /// Declare that the starting point directory should be treated as if it was a root file
    /// system.
    ///
    /// That is, absolute symlinks will use this directory as a starting point.
    pub const fn resolve_in_root(mut self, resolve_in_root: bool) -> Self {
        self.resolve_in_root = resolve_in_root;
        self
    }

    /// Perform the path creation starting a the directory `dfd`.
    pub fn create_at<D, P>(&self, dfd: &D, path: P) -> io::Result<OwnedFd>
    where
        D: ?Sized + AsRawFd,
        P: AsRef<Path>,
    {
        self.create_at_raw(dfd.as_raw_fd(), path.as_ref())
    }

    /// Perform the path creation, see [`create_at`](CreatePath::create_at()).
    fn create_at_raw(&self, dfd: RawFd, path: &Path) -> io::Result<OwnedFd> {
        let mut at_fd = dfd.as_raw_fd();
        let mut at_owned = None;
        for component in path.components() {
            match component {
                Component::Normal(name) => {
                    let name = crate::c_path::io_c_os_str(name)?;
                    match self.create_at_do(at_fd, &name) {
                        Ok(()) => (),
                        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => (),
                        Err(err) => io_bail!("error creating path component {name:?}: {err}"),
                    }
                    let next = OpenHow::new_directory()
                        .no_final_symlink(!self.allow_symlinks)
                        .resolve_in_root(self.resolve_in_root)
                        .open_at_raw(at_fd, &name)?;
                    at_fd = next.as_raw_fd();
                    at_owned = Some(next);
                }
                Component::RootDir => io_bail!("rootdir in relative CreatePath call forbidden"),
                Component::CurDir => (),
                Component::ParentDir => io_bail!("parent directory reference in CreatePath call"),
                other => io_bail!("invalid path component ({other:?})"),
            }
        }
        at_owned.ok_or_else(|| io_format_err!("CreatePath with empty path?"))
    }

    /// The raw `mkdirat()` call.
    fn create_at_do(&self, dfd: RawFd, path: &CStr) -> io::Result<()> {
        let rc = unsafe { libc::mkdirat(dfd, path.as_ptr(), self.mode) };
        if rc != 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
