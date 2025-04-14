//! Stat files with the more modern `statx(2)` call.

use std::error::Error as StdError;
use std::ffi::{CStr, c_int, c_uint};
use std::fmt;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};

use crate::CPath;
use crate::error::io_bail_last;
use crate::mount_types::{MountId, ReusedMountId};
use crate::types::Device;

const STATX_MNT_ID_UNIQUE: u32 = 0x00004000;
const STATX_SUBVOL: u32 = 0x00008000;

/// A builder for which information to query in a `statx(2)` call.
#[derive(Clone, Copy, Debug)]
pub struct Stat<'a> {
    mask: c_uint,
    at_flags: c_int,
    fd: Option<BorrowedFd<'a>>,
}

impl Default for Stat<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl Stat<'static> {
    /// Create a `Stat` request with the "basic" stats (`STATX_BASIC_STATS`) set.
    pub const fn new() -> Self {
        Self {
            mask: libc::STATX_BASIC_STATS,
            at_flags: 0,
            fd: None,
        }
    }

    /// Create an empty `Stat` request.
    ///
    /// The resulting `Metadata` will return `None` for all methods returning an `Option`.
    /// Methods *not* returning an `Option` will work as expected.
    pub const fn new_empty() -> Self {
        Self {
            mask: 0,
            at_flags: 0,
            fd: None,
        }
    }
}

macro_rules! impl_mask {
    ($(
            $(#[$doc:meta])+
            $name:ident : $value:expr
    ),+ $(,)?) => {
        $(
            $(#[$doc])+
            pub fn $name(self, on: bool) -> Self {
                self.set_mask(on, $value)
            }
        )+
    };
}

impl Stat<'_> {
    /// Set the root/beneath file descriptor the stat call should be relative to.
    pub fn at_fd<F>(self, fd: &F) -> Stat
    where
        F: ?Sized + AsFd,
    {
        Stat {
            mask: self.mask,
            at_flags: self.at_flags,
            fd: Some(fd.as_fd()),
        }
    }

    /// Update the request mask.
    fn set_mask(mut self, on: bool, value: c_uint) -> Self {
        if on {
            self.mask |= value;
        } else {
            self.mask &= !value;
        }
        self
    }

    impl_mask! {
        /// Request the file type.
        file_type : libc::STATX_TYPE,

        /// Request the file mode bits.
        mode : libc::STATX_MODE,

        /// Request the number of hard links.
        nlink : libc::STATX_NLINK,

        /// Request the owning user id.
        uid : libc::STATX_UID,

        /// Request the owning group id.
        gid : libc::STATX_GID,

        /// Request the access time.
        atime : libc::STATX_ATIME,

        /// Request the modification time.
        mtime : libc::STATX_MTIME,

        /// Request the last status change time.
        ctime : libc::STATX_CTIME,

        /// Request the inode number.
        inode : libc::STATX_INO,

        /// Request the size.
        size : libc::STATX_SIZE,

        /// Request the blocks.
        blocks : libc::STATX_BLOCKS,

        /// Request all the basic stats (file type, mode, hard link count, uid, gid, atime, mtime,
        /// ctime, inode, size, blocks).
        ///
        /// This is the equivalent of `STATX_BASIC_STATS`.
        basic_stats : libc::STATX_BASIC_STATS,

        /// Request the creation time.
        btime : libc::STATX_BTIME,

        /// Request the *reused* mount id the path resides on. (Kernel version 5.7)
        reused_mount_id : libc::STATX_MNT_ID,

        /// Request the *unique* mount id the path resides on. (Kernel version 6.9)
        unique_mount_id : STATX_MNT_ID_UNIQUE,

        /// Request direct I/O alignment information.
        dio_align : libc::STATX_DIOALIGN,

        /// Request the subvolume id. (Kernel version 6.11)
        subvol : STATX_SUBVOL,

        /// Request everything.
        all : libc::STATX_ALL,
    }

    /// Modify the `AT_*` flags.
    fn set_at_flags(mut self, on: bool, value: c_int) -> Self {
        if on {
            self.at_flags |= value;
        } else {
            self.at_flags &= !value;
        }
        self
    }

    /// Don't perform auto-mounting for this `stat` call.
    pub fn no_auto_mount(self, on: bool) -> Self {
        self.set_at_flags(on, libc::AT_NO_AUTOMOUNT)
    }

    /// Do not dereference a final symlink.
    pub fn no_final_symlink(self, on: bool) -> Self {
        self.set_at_flags(on, libc::AT_SYMLINK_NOFOLLOW)
    }

    /// Use the same `sync` behavior as a regular `stat(2)` call.
    pub fn sync_as_stat(self, on: bool) -> Self {
        self.set_at_flags(on, libc::AT_STATX_SYNC_AS_STAT)
    }

    /// Force a `sync` before performing this `statx(2)` call.
    pub fn force_sync(self, on: bool) -> Self {
        self.set_at_flags(on, libc::AT_STATX_FORCE_SYNC)
    }

    /// Do not `sync` before performing this `statx(2)` call.
    ///
    /// File information may be out of date.
    pub fn no_sync(self, on: bool) -> Self {
        self.set_at_flags(on, libc::AT_STATX_DONT_SYNC)
    }

    /// Perform a stat on the currently selected file descriptor *itself*.
    pub fn stat_fd(self) -> io::Result<Metadata> {
        self.set_at_flags(true, libc::AT_EMPTY_PATH).stat("")
    }

    /// Perform a stat. For a relative path, it will be relative to the currently selected file
    /// descriptor.
    pub fn stat<P: ?Sized + CPath>(self, path: &P) -> io::Result<Metadata> {
        path.c_path(|path| self.stat_raw(path))?
    }

    /// The raw `statx()` implementation.
    fn stat_raw(self, path: &CStr) -> io::Result<Metadata> {
        let mut data: CStatx;
        let rc = unsafe {
            data = std::mem::zeroed();
            libc::statx(
                self.fd.map(|fd| fd.as_raw_fd()).unwrap_or(-libc::EBADF),
                path.as_ptr(),
                self.at_flags,
                self.mask,
                &raw mut data as *mut libc::statx,
            )
        };
        if rc != 0 {
            io_bail_last!();
        }
        Ok(Metadata::from(data))
    }
}

/// The result of a `statx(2)` operation via [`Stat`].
#[derive(Clone)]
pub struct Metadata {
    data: CStatx,
}

impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Metadata")
            .field("stx_mask", &self.data.stx_mask)
            .field("stx_blksize", &self.data.stx_blksize)
            .field("stx_attributes", &self.data.stx_attributes)
            .field("stx_nlink", &self.data.stx_nlink)
            .field("stx_uid", &self.data.stx_uid)
            .field("stx_gid", &self.data.stx_gid)
            .field("stx_mode", &self.data.stx_mode)
            .field("stx_ino", &self.data.stx_ino)
            .field("stx_size", &self.data.stx_size)
            .field("stx_blocks", &self.data.stx_blocks)
            .field("stx_attributes_mask", &self.data.stx_attributes_mask)
            .field("stx_atime.tv_sec", &self.data.stx_atime.tv_sec)
            .field("stx_atime.tv_nsec", &self.data.stx_atime.tv_nsec)
            .field("stx_btime.tv_sec", &self.data.stx_btime.tv_sec)
            .field("stx_btime.tv_nsec", &self.data.stx_btime.tv_nsec)
            .field("stx_ctime.tv_sec", &self.data.stx_ctime.tv_sec)
            .field("stx_ctime.tv_nsec", &self.data.stx_ctime.tv_nsec)
            .field("stx_mtime.tv_sec", &self.data.stx_mtime.tv_sec)
            .field("stx_mtime.tv_nsec", &self.data.stx_mtime.tv_nsec)
            .field("stx_rdev_major", &self.data.stx_rdev_major)
            .field("stx_rdev_minor", &self.data.stx_rdev_minor)
            .field("stx_dev_major", &self.data.stx_dev_major)
            .field("stx_dev_minor", &self.data.stx_dev_minor)
            .field("stx_mnt_id", &self.data.stx_mnt_id)
            .field("stx_dio_mem_align", &self.data.stx_dio_mem_align)
            .field("stx_dio_offset_align", &self.data.stx_dio_offset_align)
            .finish()
    }
}

impl From<CStatx> for Metadata {
    fn from(data: CStatx) -> Self {
        Self { data }
    }
}

impl From<libc::statx> for Metadata {
    fn from(data: libc::statx) -> Self {
        // The linux headers include `__spare` fields for these, but there were only 9 left when I
        // copied them for `CStatx` so...
        let data = unsafe {
            let size = std::mem::size_of::<libc::statx>().min(std::mem::size_of::<CStatx>());
            let mut my_data: CStatx = std::mem::zeroed();
            std::ptr::copy(
                &raw const data as *const u8,
                &raw mut my_data as *mut u8,
                size,
            );
            my_data
        };
        Self::from(data)
    }
}

impl Metadata {
    /// Mask a value.
    fn maybe<T: Copy>(&self, mask: c_uint, value: T) -> Option<T> {
        (self.data.stx_mask & mask != 0).then_some(value)
    }

    /// Return the block size for the file system this file resides on.
    pub fn block_size(&self) -> u32 {
        self.data.stx_blksize
    }

    /// Indicates that the file is compressed.
    pub fn is_compressed(&self) -> Option<bool> {
        (self.data.stx_attributes_mask & (libc::STATX_ATTR_COMPRESSED as u64) != 0)
            .then_some(self.data.stx_attributes & (libc::STATX_ATTR_COMPRESSED as u64) != 0)
    }

    /// Indicates that the file is immutable.
    pub fn is_immutable(&self) -> Option<bool> {
        (self.data.stx_attributes_mask & (libc::STATX_ATTR_IMMUTABLE as u64) != 0)
            .then_some(self.data.stx_attributes & (libc::STATX_ATTR_IMMUTABLE as u64) != 0)
    }

    /// Indicates that the file only allows opening in append mode.
    pub fn is_append_only(&self) -> Option<bool> {
        (self.data.stx_attributes_mask & (libc::STATX_ATTR_APPEND as u64) != 0)
            .then_some(self.data.stx_attributes & (libc::STATX_ATTR_APPEND as u64) != 0)
    }

    /// Indicates the file is not a candidate for backup when `dump(8)` runs.
    pub fn is_no_dump(&self) -> Option<bool> {
        (self.data.stx_attributes_mask & (libc::STATX_ATTR_NODUMP as u64) != 0)
            .then_some(self.data.stx_attributes & (libc::STATX_ATTR_NODUMP as u64) != 0)
    }

    /// Indicates the file is encrypted.
    pub fn is_encrypted(&self) -> Option<bool> {
        (self.data.stx_attributes_mask & (libc::STATX_ATTR_ENCRYPTED as u64) != 0)
            .then_some(self.data.stx_attributes & (libc::STATX_ATTR_ENCRYPTED as u64) != 0)
    }

    /// Indicates this path is an automount trigger.
    pub fn is_automount(&self) -> Option<bool> {
        (self.data.stx_attributes_mask & (libc::STATX_ATTR_AUTOMOUNT as u64) != 0)
            .then_some(self.data.stx_attributes & (libc::STATX_ATTR_AUTOMOUNT as u64) != 0)
    }

    /// Indicates this path is the root node of a mount point.
    ///
    /// This only fails on older kernels which do not know about this flag.
    ///
    /// Starting with kernel version 5.7, this will always return `Some`.
    ///
    /// See linux kernel commit `80340fe3605c0e78 ("statx: add mount_root")`.
    pub fn is_mount_root(&self) -> Option<bool> {
        (self.data.stx_attributes_mask & (libc::STATX_ATTR_MOUNT_ROOT as u64) != 0)
            .then_some(self.data.stx_attributes & (libc::STATX_ATTR_MOUNT_ROOT as u64) != 0)
    }

    /// Indicates the file is verity protected.
    pub fn is_verity(&self) -> Option<bool> {
        (self.data.stx_attributes_mask & (libc::STATX_ATTR_VERITY as u64) != 0)
            .then_some(self.data.stx_attributes & (libc::STATX_ATTR_VERITY as u64) != 0)
    }

    /// Indicates the file is currently in DAX state.
    pub fn is_dax(&self) -> Option<bool> {
        (self.data.stx_attributes_mask & (libc::STATX_ATTR_DAX as u64) != 0)
            .then_some(self.data.stx_attributes & (libc::STATX_ATTR_DAX as u64) != 0)
    }

    /// Get the hard link count.
    pub fn hard_links(&self) -> Option<u32> {
        self.maybe(libc::STATX_NLINK, self.data.stx_nlink)
    }

    /// Get the owning user id.
    pub fn uid(&self) -> Option<u32> {
        self.maybe(libc::STATX_UID, self.data.stx_uid)
    }

    /// Get the owning group id.
    pub fn gid(&self) -> Option<u32> {
        self.maybe(libc::STATX_GID, self.data.stx_gid)
    }

    /// The file type related mode bits.
    pub fn file_type(&self) -> Option<u16> {
        self.maybe(libc::STATX_MODE, self.data.stx_mode & (libc::S_IFMT as u16))
    }

    /// The mode bits *without* the file type.
    pub fn file_mode(&self) -> Option<u16> {
        self.maybe(
            libc::STATX_MODE,
            self.data.stx_mode & ((libc::S_IFMT - 1) as u16),
        )
    }

    /// Get the inode number.
    pub fn inode(&self) -> Option<u64> {
        self.maybe(libc::STATX_INO, self.data.stx_ino)
    }

    /// Get the size in bytes.
    pub fn size(&self) -> Option<u64> {
        self.maybe(libc::STATX_SIZE, self.data.stx_size)
    }

    /// Get the block count.
    pub fn blocks(&self) -> Option<u64> {
        self.maybe(libc::STATX_BLOCKS, self.data.stx_blocks)
    }

    /// Get the access time.
    pub fn atime(&self) -> Option<Timestamp> {
        self.maybe(libc::STATX_ATIME, self.data.stx_atime.into())
    }

    /// Get the creation time.
    pub fn btime(&self) -> Option<Timestamp> {
        self.maybe(libc::STATX_BTIME, self.data.stx_btime.into())
    }

    /// Get the last status change time.
    pub fn ctime(&self) -> Option<Timestamp> {
        self.maybe(libc::STATX_CTIME, self.data.stx_ctime.into())
    }

    /// Get the last modification time.
    pub fn mtime(&self) -> Option<Timestamp> {
        self.maybe(libc::STATX_MTIME, self.data.stx_mtime.into())
    }

    /// This is the device info if this is a character or block device.
    pub fn device(&self) -> Option<Device> {
        let ty = self.file_type()? as u32;
        if ty == libc::S_IFCHR || ty == libc::S_IFBLK {
            Some(Device {
                major: self.data.stx_rdev_major,
                minor: self.data.stx_rdev_minor,
            })
        } else {
            None
        }
    }

    /// This is the device this file resides on.
    pub fn fs_device(&self) -> Device {
        Device {
            major: self.data.stx_dev_major,
            minor: self.data.stx_dev_minor,
        }
    }

    /// Get the *reused* mount id this file resides on, this *fails* if the *unique* mount ID was
    /// also requested, or the kernel was too old.
    ///
    /// This only fails on older kernels which do not know about this flag.
    ///
    /// Starting with kernel version 5.7, this will always return `Some`.
    ///
    /// See linux kernel commit `fa2fcf4f1df1559a` ("statx: add mount ID")`.
    pub fn reused_mount_id(&self) -> Result<ReusedMountId, ReusedMountIdUnavailable> {
        if self.data.stx_mask & STATX_MNT_ID_UNIQUE != 0 {
            Err(ReusedMountIdUnavailable::UniqueIdAvailable(
                MountId::from_raw(self.data.stx_mnt_id),
            ))
        } else if self.data.stx_mask & libc::STATX_MNT_ID != 0 {
            Ok(ReusedMountId::from_raw(self.data.stx_mnt_id as u32))
        } else {
            Err(ReusedMountIdUnavailable::Unavailable)
        }
    }

    /// Get the *unique* mount id this file resides on.
    ///
    /// This only fails on older kernels which do not know about this flag.
    ///
    /// Starting with kernel version 6.9, this will always return `Some`.
    ///
    /// See linux kernel commit `98d2b43081972abe` ("add unique mount ID")`.
    pub fn unique_mount_id(&self) -> Option<MountId> {
        self.maybe(STATX_MNT_ID_UNIQUE, self.data.stx_mnt_id)
            .map(MountId::from_raw)
    }

    /// Memory buffer alignment for direct I/O.
    pub fn dio_mem_align(&self) -> Option<u32> {
        self.maybe(libc::STATX_DIOALIGN, self.data.stx_dio_mem_align)
    }

    /// File offset alignment for direct I/O.
    pub fn dio_offset_align(&self) -> Option<u32> {
        self.maybe(libc::STATX_DIOALIGN, self.data.stx_dio_offset_align)
    }

    /// Get the subvolume ID this file resides on.
    ///
    /// These are the IDs used in `btrfs` and `bcachefs`.
    ///
    /// This was introduced in kernel verison 6.11.
    ///
    /// See linux kernel commit `2a82bb02941fb53d` ("statx: stx_subvol").
    pub fn subvolume_id(&self) -> Option<u64> {
        self.maybe(STATX_SUBVOL, self.data.stx_subvol)
    }
}

/// A time stamp returned in a `statx(2)` call.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Timestamp {
    /// The seconds since the epoch.
    pub sec: i64,
    /// The nanoseconds since the current second.
    pub nsec: u32,
}

impl From<libc::statx_timestamp> for Timestamp {
    fn from(t: libc::statx_timestamp) -> Self {
        Self {
            sec: t.tv_sec,
            nsec: t.tv_nsec,
        }
    }
}

/// An error querying the [`ReusedMountId] id of a [`Stat`] call can either be that it was not
/// included in the request, the kernel was too old, or the *unique* id was requested.
#[derive(Clone, Copy, Debug)]
pub enum ReusedMountIdUnavailable {
    /// The id was not requested or the kernel was too old.
    Unavailable,
    /// The *unique* id was requested.
    UniqueIdAvailable(MountId),
}

impl StdError for ReusedMountIdUnavailable {}

impl fmt::Display for ReusedMountIdUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Unavailable => f.write_str("mount id not requested or kernel too old"),
            Self::UniqueIdAvailable(_) => f.write_str("unique mount id replaces reused mount id"),
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct CStatx {
    stx_mask: u32,
    stx_blksize: u32,
    stx_attributes: u64,
    stx_nlink: u32,
    stx_uid: u32,
    stx_gid: u32,
    stx_mode: u16,
    __spare0: u16,
    stx_ino: u64,
    stx_size: u64,
    stx_blocks: u64,
    stx_attributes_mask: u64,
    stx_atime: libc::statx_timestamp,
    stx_btime: libc::statx_timestamp,
    stx_ctime: libc::statx_timestamp,
    stx_mtime: libc::statx_timestamp,
    stx_rdev_major: u32,
    stx_rdev_minor: u32,
    stx_dev_major: u32,
    stx_dev_minor: u32,
    stx_mnt_id: u64,
    stx_dio_mem_align: u32,
    stx_dio_offset_align: u32,
    stx_subvol: u64,
    stx_atomic_write_unit_min: u32,
    stx_atomic_write_unit_max: u32,
    stx_atomic_write_segments_max: u32,
    stx_dio_read_offset_align: u32,
    __spare3: [u64; 9],
}
