//! Higher level `openat2` interface.

use std::ffi::CStr;
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};

use crate::CPath;

/// Directory / base file descriptor which enforces that the path provided to a `*at()` functions
/// must bee an absolute path.
///
/// This uses `-EBADF` as a file descriptor. While this could technically use `-1`, `-EBADF` will
/// be much more explicit when debugging/stracing a program using this.
#[derive(Clone, Copy, Debug)]
pub struct AbsolutePath;

impl AsRawFd for AbsolutePath {
    fn as_raw_fd(&self) -> RawFd {
        -libc::EBADF
    }
}

impl AsFd for AbsolutePath {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(-libc::EBADF) }
    }
}

/// The kernel's `struct open_how`.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct RawOpenHow {
    /// The `O_*` flags used to open a file.
    pub flags: u64,
    /// The `mode` bits used to create a new file.
    pub mode: u64,
    /// The resolve flags for how to resolve a path in the `openat2()` call.
    pub resolve: u64,
}

impl Default for RawOpenHow {
    fn default() -> Self {
        Self::new()
    }
}

impl RawOpenHow {
    /// Create a default `RawOpenHow` struct with the `CLOEXEC` and `NOCTTY` flags set and a mode of
    /// `0o000`.
    pub const fn new() -> Self {
        Self {
            flags: libc::O_CLOEXEC as u64,
            mode: 0,
            resolve: 0,
        }
    }

    /// Create an empty `RawOpenHow` struct.
    pub const fn new_empty() -> Self {
        Self {
            flags: 0,
            mode: 0,
            resolve: 0,
        }
    }
}

/// A "builder" style `openat2(2)` interface.
#[derive(Clone, Copy, Debug)]
pub struct OpenHow<'a> {
    /// The raw `struct open_how`.
    pub how: RawOpenHow,

    /// An optional file descriptor use for relative (or `chroot`-like) access.
    pub fd: Option<BorrowedFd<'a>>,
}

/// The default implementation implies the `CLOEXEC` and `NOCTTY` flag.
impl Default for OpenHow<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenHow<'static> {
    /// Create a default `OpenHow` instance which is completely empty, *not*  implying `O_CLOEXEC`.
    pub const fn new_empty() -> Self {
        Self {
            how: RawOpenHow::new_empty(),
            fd: None,
        }
    }

    /// Create a default `OpenHow` instance with the `CLOEXEC` and `NOCTTY` flags set and a mode of `0o000`.
    pub const fn new() -> Self {
        Self {
            how: RawOpenHow::new(),
            fd: None,
        }
    }

    /// Create a new default `OpenHow` with the flags set to `O_RDONLY | O_CLOEXEC | O_NOCTTY`.
    pub const fn new_read() -> Self {
        let mut how = RawOpenHow::new();
        how.flags |= libc::O_RDONLY as u64;
        Self { how, fd: None }
    }

    /// Create a new default `OpenHow` with the flags set to `O_WRONLY | O_CLOEXEC | O_NOCTTY`.
    pub const fn new_write() -> Self {
        let mut how = RawOpenHow::new();
        how.flags |= libc::O_WRONLY as u64;
        Self { how, fd: None }
    }

    /// Create a new default `OpenHow` with the flags set to `O_RDWR | O_CLOEXEC | O_NOCTTY`.
    pub const fn new_rw() -> Self {
        let mut how = RawOpenHow::new();
        how.flags |= libc::O_RDWR as u64;
        Self { how, fd: None }
    }

    /// Create a new default `OpenHow` with the flags set to `O_DIRECTORY | O_CLOEXEC | O_NOCTTY`.
    pub const fn new_directory() -> Self {
        let mut how = RawOpenHow::new();
        how.flags |= libc::O_DIRECTORY as u64;
        Self { how, fd: None }
    }
}

impl OpenHow<'_> {
    /// Set or clear a set of flags.
    pub fn set_flags(mut self, on: bool, flags: u64) -> Self {
        if on {
            self.how.flags |= flags;
        } else {
            self.how.flags &= !flags;
        }
        self
    }

    /// Set or clear a resolve flag.
    fn set_resolve(mut self, on: bool, resolve: u64) -> Self {
        if on {
            self.how.resolve |= resolve;
        } else {
            self.how.resolve &= !resolve;
        }
        self
    }

    /// Resolve only beneath the passed file descriptor.
    pub fn resolve_beneath(self, on: bool) -> Self {
        self.set_resolve(on, libc::RESOLVE_BENEATH)
    }

    /// Treat the passed directory file descriptor as the file system root.
    pub fn resolve_in_root(self, on: bool) -> Self {
        self.set_resolve(on, libc::RESOLVE_IN_ROOT)
    }

    /// Set the root/beneath file descriptor.
    pub fn at_fd<F>(self, fd: &F) -> OpenHow
    where
        F: ?Sized + AsFd,
    {
        OpenHow {
            how: self.how,
            fd: Some(fd.as_fd()),
        }
    }

    /// Set the root/beneath file descriptor.
    ///
    /// # Safety
    ///
    /// It is the caller's responsibility to ensure the file descriptor remains valid until the
    /// `OpenHow` is used up.
    pub unsafe fn at_fd_raw(self, fd: RawFd) -> OpenHow<'static> {
        OpenHow {
            how: self.how,
            fd: Some(unsafe { BorrowedFd::borrow_raw(fd) }),
        }
    }

    /// Disallow magic link resolution (eg. files from `/proc` that magically resolve to specific
    /// resources).
    pub fn resolve_no_magiclinks(self, on: bool) -> Self {
        self.set_resolve(on, libc::RESOLVE_NO_MAGICLINKS)
    }

    /// Disallow resolving symlinks generally *everywhere* in the provided path.
    ///
    /// Note that this is not the same as [`no_final_symlink`](Self::no_final_symlink()).
    pub fn resolve_no_symlinks(self, on: bool) -> Self {
        self.set_resolve(on, libc::RESOLVE_NO_SYMLINKS)
    }

    /// Disallow crossing file system boundaries (including bind mounts).
    pub fn resolve_no_xdev(self, on: bool) -> Self {
        self.set_resolve(on, libc::RESOLVE_NO_XDEV)
    }

    /// Make the operation fail unless it can be served from the kernel's cache.
    pub fn resolve_cached_only(self, on: bool) -> Self {
        self.set_resolve(on, libc::RESOLVE_CACHED)
    }

    /// Change the file mode for when creating files.
    pub fn mode(mut self, mode: u64) -> Self {
        self.how.mode = mode;
        self
    }

    /// Add custom `O_*` flags.
    pub fn flags(self, flags: u64) -> Self {
        self.set_flags(true, flags)
    }

    /// Require the path to be a directory.
    pub fn directory(self, on: bool) -> Self {
        self.set_flags(on, libc::O_DIRECTORY as u64)
    }

    /// Create the file if it does not exist.
    pub fn create(self, on: bool) -> Self {
        self.set_flags(on, libc::O_CREAT as u64)
    }

    /// Fail if a file to be created already exists.
    ///
    /// In case of creating a temp file with `O_TMPFILE` ensure that it cannot be linked.
    ///
    /// Sets the `O_EXCL` flag.
    pub fn fail_if_exists(self, on: bool) -> Self {
        self.set_flags(on, libc::O_EXCL as u64)
    }

    /// Truncate the file if it does exist.
    pub fn truncate(self, on: bool) -> Self {
        self.set_flags(on, libc::O_TRUNC as u64)
    }

    /// Disallow the *final* path component to be a symlink.
    ///
    /// This sets the `O_NOFOLLOW` flag which only affects the *final* path component, but symlinks
    /// in directories on the way there will still be resolved. Should this be a problem, the
    /// [`resolve_no_magiclinks`](Self::resolve_no_magiclinks) flag can be used instead.
    pub fn no_final_symlink(self, on: bool) -> Self {
        self.set_flags(on, libc::O_NOFOLLOW as u64)
    }

    /// Open for appending.
    pub fn append(self, on: bool) -> Self {
        self.set_flags(on, libc::O_APPEND as u64)
    }
}

impl OpenHow<'_> {
    /// Open the path.
    pub fn open<P>(&self, path: &P) -> io::Result<OwnedFd>
    where
        P: ?Sized + CPath,
    {
        path.c_path(|path| self.open_raw(path))?
    }

    /// This is [`open`](OpenHow::open()) with raw parameters.
    pub fn open_raw(&self, path: &CStr) -> io::Result<OwnedFd> {
        self.open_at_raw(
            self.fd.map(|fd| fd.as_raw_fd()).unwrap_or(libc::AT_FDCWD),
            path,
        )
    }

    /// This calls `openat2` with raw parameters.
    pub fn open_at_raw(&self, dirfd: RawFd, path: &CStr) -> io::Result<OwnedFd> {
        let res = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                dirfd,
                path.as_ptr(),
                &self.how,
                std::mem::size_of_val(&self.how),
            )
        };

        if res < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { OwnedFd::from_raw_fd(res as RawFd) })
    }

    /// Open a file.
    pub fn open_file<P>(&self, path: &P) -> io::Result<File>
    where
        P: ?Sized + CPath,
    {
        let fd = self.open(path)?;
        Ok(unsafe { File::from_raw_fd(fd.into_raw_fd()) })
    }
}
