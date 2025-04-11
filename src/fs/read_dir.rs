//! Read directories with `getdents64(2)`.

use std::ffi::{CStr, OsStr, OsString, c_char, c_uchar, c_ushort};
use std::io;
use std::mem::{align_of, offset_of};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};

use crate::CPath;
use crate::error::{io_bail_last, io_format_err};
use crate::open::OpenHow;

/// Iterate through the contents of a directory, see [`ReadDir`].
///
/// Before opening the directory, the `O_RDWR | O_WRONLY | O_CREAT` flags are dropped
/// automatically.
/// The `O_RDONLY | O_DIRECTORY` flags are added.
pub fn read_dir<P: ?Sized + CPath>(how: OpenHow, path: &P) -> io::Result<ReadDir> {
    how.set_flags(
        false,
        (libc::O_RDWR | libc::O_WRONLY | libc::O_CREAT) as u64,
    )
    .set_flags(true, (libc::O_RDONLY | libc::O_DIRECTORY) as u64)
    .open(path)
    .map(ReadDir::new)
}

/// An iterator through the contents of a directory (skipping `.` and `..` automatically).
pub struct ReadDir {
    inner: GetDEnts,
}

impl ReadDir {
    /// Open a directory for iteration.
    pub fn read<P: ?Sized + CPath>(path: &P) -> io::Result<ReadDir> {
        Ok(Self::new(OpenHow::new_read().directory(true).open(path)?))
    }

    /// Open a directory for iteration.
    pub fn read_at<F, P>(dirfd: &F, path: &P) -> io::Result<ReadDir>
    where
        P: ?Sized + CPath,
        F: ?Sized + AsFd,
    {
        Ok(Self::new(
            OpenHow::new_read()
                .directory(true)
                .at_fd(dirfd)
                .open(path)?,
        ))
    }

    /// Open a directory for iteration.
    pub fn read_at_raw(dirfd: RawFd, path: &CStr) -> io::Result<ReadDir> {
        Ok(Self::new(
            OpenHow::new_read()
                .directory(true)
                .open_at_raw(dirfd, path)?,
        ))
    }

    fn new(fd: OwnedFd) -> ReadDir {
        Self {
            inner: GetDEnts::new(fd),
        }
    }
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEnt>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl AsRawFd for ReadDir {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl AsFd for ReadDir {
    fn as_fd(&self) -> BorrowedFd {
        self.inner.as_fd()
    }
}

/// A directory entry yielded by the [`ReadDir`] iterator.
#[derive(Clone, Debug)]
pub struct DirEnt {
    inner: LinuxDirent64,
    name: OsString,
}

impl DirEnt {
    /// Get the name of this entry.
    pub fn name(&self) -> &OsStr {
        &self.name
    }

    /// If only the name is of interest, we can move out the allocated string.
    pub fn into_name(self) -> OsString {
        self.name
    }

    /// Get the file type *without* an additional `stat` call, but only if the file system supports
    /// it.
    pub fn entry_type(&self) -> Option<EntryType> {
        EntryType::from_raw(self.inner.d_type)
    }
}

/// The type of an entry in a directory listing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum EntryType {
    /// A block device.
    Block,
    /// A character device.
    Char,
    /// A directory.
    Dir,
    /// A FIFO.
    Fifo,
    /// A symlink.
    Link,
    /// A regular file.
    Regular,
    /// A unix socket.
    Sock,
}

impl EntryType {
    fn from_raw(d_type: c_uchar) -> Option<Self> {
        Some(match d_type {
            libc::DT_UNKNOWN => return None,
            libc::DT_BLK => Self::Block,
            libc::DT_CHR => Self::Char,
            libc::DT_DIR => Self::Dir,
            libc::DT_FIFO => Self::Fifo,
            libc::DT_LNK => Self::Link,
            libc::DT_REG => Self::Regular,
            libc::DT_SOCK => Self::Sock,
            // Should this error?
            _ => return None,
        })
    }

    /// Convenience method to check for `EntryType::Block`.
    pub const fn is_block(self) -> bool {
        matches!(self, Self::Block)
    }

    /// Convenience method to check for `EntryType::Char`.
    pub const fn is_char(self) -> bool {
        matches!(self, Self::Char)
    }

    /// Convenience method to check for `EntryType::Dir`.
    pub const fn is_dir(self) -> bool {
        matches!(self, Self::Dir)
    }

    /// Convenience method to check for `EntryType::Fifo`.
    pub const fn is_fifo(self) -> bool {
        matches!(self, Self::Fifo)
    }

    /// Convenience method to check for `EntryType::Link`.
    pub const fn is_link(self) -> bool {
        matches!(self, Self::Link)
    }

    /// Convenience method to check for `EntryType::Regular`.
    pub const fn is_regular(self) -> bool {
        matches!(self, Self::Regular)
    }

    /// Convenience method to check for `EntryType::Sock`.
    pub const fn is_sock(self) -> bool {
        matches!(self, Self::Sock)
    }
}

struct GetDEnts {
    fd: OwnedFd,
    buf: Box<[u8]>,
    have: usize,
    at: usize,
    eof: bool,
}

impl GetDEnts {
    fn new(fd: OwnedFd) -> Self {
        Self {
            fd,
            buf: crate::bytes::uninitialized(4096, align_of::<LinuxDirent64>()),
            have: 0,
            at: 0,
            eof: false,
        }
    }

    fn available(&self) -> usize {
        self.have.saturating_sub(self.at)
    }

    /// Returns `self.available()`.
    fn maybe_read_more(&mut self) -> io::Result<usize> {
        let available = self.available();
        if available > DIRENT_SIZE {
            return Ok(available);
        }

        self.at = 0;
        self.have = 0;

        let rc = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                self.fd.as_raw_fd(),
                self.buf.as_mut_ptr(),
                self.buf.len(),
            )
        };
        if rc < 0 {
            io_bail_last!();
        }
        self.have = rc as usize;
        if self.have == 0 {
            self.eof = true;
        }
        Ok(self.have)
    }
}

impl Iterator for GetDEnts {
    type Item = io::Result<DirEnt>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let available = match self.maybe_read_more() {
                Err(err) => return Some(Err(err)),
                Ok(0) => return None,
                Ok(available) => available,
            };
            assert!(available > DIRENT_SIZE);

            let out = unsafe {
                let inner = std::ptr::read_unaligned(
                    &self.buf[self.at] as *const u8 as *const LinuxDirent64,
                );
                let at = self.at;
                self.at += usize::from(inner.d_reclen);

                let name = {
                    let rec_end = at + usize::from(inner.d_reclen);
                    if rec_end > self.have {
                        self.eof = true;
                        return Some(Err(io_format_err!(
                            "kernel returned excessive record length"
                        )));
                    }
                    let name_start = at + std::mem::offset_of!(LinuxDirent64, d_name);
                    if name_start > rec_end {
                        return Some(Err(io_format_err!(
                            "kernel returned record smaller than than the data"
                        )));
                    }

                    let sub_buf = &self.buf[name_start..rec_end];
                    match sub_buf.iter().position(|&b| b == 0) {
                        Some(len) => &sub_buf[..len],
                        None => {
                            return Some(Err(io_format_err!("dentry without terminating zero")));
                        }
                    }
                };
                if name == b"." || name == b".." {
                    continue;
                }
                let name = OsStr::from_encoded_bytes_unchecked(name).to_owned();

                DirEnt { inner, name }
            };

            return Some(Ok(out));
        }
    }
}

const DIRENT_SIZE: usize = offset_of!(LinuxDirent64, d_name);

#[derive(Clone, Debug)]
#[repr(C)]
struct LinuxDirent64 {
    d_ino: u64,
    d_off: i64,
    d_reclen: c_ushort,
    d_type: c_uchar,
    d_name: c_char,
}

impl AsRawFd for GetDEnts {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl AsFd for GetDEnts {
    fn as_fd(&self) -> BorrowedFd {
        self.fd.as_fd()
    }
}
