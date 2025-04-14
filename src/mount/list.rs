//! `listmount(2)` and `statmount(2)` implementation.

use std::ffi::CStr;
use std::io;

use crate::error::io_assert;
use crate::types::Device;

use super::sys::{MountAttr, MountPropagation, StatMountFlags, SuperblockFlags};
use super::sys::{SYS_listmount, SYS_statmount};
use super::{MountId, MountNsId, ReusedMountId};

/// Structure for passing mount ID and miscellaneous parameters to `statmount(2)` and
/// `listmount(2)`.
///
/// Originally introduced in kernel 6.9.
/// This contains the `mnt_ns_id` field introduced in kernel 6.10.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct RawMountIdRequest {
    /// This struct's size.
    size: u32,
    spare: u32,
    mount_id: MountId,
    /// For `statmount(2)` this is the request mask.
    /// For `listmount(2)` this is the last listed mount id (or zero).
    param: u64,
}

/// Structure for passing mount ID and miscellaneous parameters to `statmount(2)` and
/// `listmount(2)`.
///
/// This requires kernel 6.10.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct RawMountIdRequestForNs {
    req: RawMountIdRequest,
    /// This was introduced in kernel 6.10
    mnt_ns_id: MountNsId,
}

#[derive(Clone, Copy, Debug)]
/// A request for a `statmount(2)` or `listmount(2)` call.
struct MountIdRequest {
    inner: MountIdRequestInner,
}

#[derive(Clone, Copy, Debug)]
/// To support a wider range of kernels, with no `mnt_ns_id` set we use the old request size.
enum MountIdRequestInner {
    Ver0(RawMountIdRequest),
    Ver1(RawMountIdRequestForNs),
}

impl MountIdRequest {
    /// Make an empty mount id request.
    pub const fn new() -> Self {
        Self {
            inner: MountIdRequestInner::Ver0(RawMountIdRequest {
                size: std::mem::size_of::<RawMountIdRequest>() as u32,
                spare: 0,
                mount_id: MountId::from_raw(0),
                param: 0,
            }),
        }
    }

    /// Set the mount ID to request stats for or begin listing from.
    pub const fn mount_id(mut self, id: MountId) -> Self {
        self.inner = match self.inner {
            MountIdRequestInner::Ver0(mut n) => MountIdRequestInner::Ver0({
                n.mount_id = id;
                n
            }),
            MountIdRequestInner::Ver1(mut n) => MountIdRequestInner::Ver1({
                n.req.mount_id = id;
                n
            }),
        };
        self
    }

    /// Set the namespace this request is for. This bumps the kernel requirement from 6.9 to 6.10.
    pub const fn mount_namespace(mut self, mnt_ns_id: MountNsId) -> Self {
        self.inner = match self.inner {
            MountIdRequestInner::Ver0(mut req) => {
                req.size = std::mem::size_of::<RawMountIdRequestForNs>() as u32;
                MountIdRequestInner::Ver1(RawMountIdRequestForNs { req, mnt_ns_id })
            }
            MountIdRequestInner::Ver1(mut n) => MountIdRequestInner::Ver1({
                n.mnt_ns_id = mnt_ns_id;
                n
            }),
        };
        self
    }

    /// Set the size and return the raw pointer.
    fn finalize(&mut self, param: u64) -> *mut u8 {
        match &mut self.inner {
            MountIdRequestInner::Ver0(n) => {
                let size = std::mem::size_of_val::<RawMountIdRequest>(n);
                n.param = param;
                n.size = u32::try_from(size).unwrap();
                &mut *n as *mut RawMountIdRequest as *mut u8
            }
            MountIdRequestInner::Ver1(n) => {
                let size = std::mem::size_of_val::<RawMountIdRequestForNs>(n);
                n.req.param = param;
                n.req.size = u32::try_from(size).unwrap();
                &mut *n as *mut RawMountIdRequestForNs as *mut u8
            }
        }
    }
}

/// An iterator over the mount IDs inside a mount namespace.
pub struct ListMounts {
    request: MountIdRequest,
    buf: Box<[MountId; 64]>,
    at: usize,
    capacity: usize,
    mnt_id: Option<MountId>,
    done: bool,
}

/// Get an iterator over the mounts in the current namespace starting at the root.
pub fn list() -> ListMounts {
    ListMounts::here()
}

impl ListMounts {
    /// Create an iterator over mount points.
    ///
    /// This will yield the unique [`MountId`]s of all the child mounts under `parent`.
    /// To start at the top, `parent` can be `MountId::root()`.
    ///
    /// If a namespace is provided, the children from that namespace's point of view will be
    /// listed.
    pub fn new(parent: MountId, namespace: Option<MountNsId>) -> Self {
        let mut request = MountIdRequest::new().mount_id(parent);
        if let Some(namespace) = namespace {
            request = request.mount_namespace(namespace);
        }
        Self {
            request,
            buf: Box::new([MountId::from_raw(0); 64]),
            at: 0,
            capacity: 0,
            mnt_id: None,
            done: false,
        }
    }

    /// Create an iterator over the mount points of the current namespace.
    pub fn here() -> Self {
        Self::new(MountId::root(), None)
    }

    /// If we're at the end of the current list but not yet done, query the next set of mounts...
    fn list_more(&mut self) -> io::Result<()> {
        if self.done || self.at < self.capacity {
            return Ok(());
        }

        let req = self
            .request
            .finalize(self.mnt_id.map_or(0, MountId::as_raw_id));
        let rc =
            unsafe { libc::syscall(SYS_listmount, req, self.buf.as_mut_ptr(), self.buf.len(), 0) };
        io_assert!(rc >= 0);

        self.capacity = rc as usize;
        self.at = 0;
        if self.capacity == 0 {
            self.done = true;
        } else {
            self.mnt_id = Some(self.buf[self.capacity - 1]);
        }
        Ok(())
    }
}

impl Iterator for ListMounts {
    type Item = io::Result<MountId>;

    fn next(&mut self) -> Option<io::Result<MountId>> {
        if let Err(err) = self.list_more() {
            return Some(Err(err));
        }

        if self.done {
            return None;
        }

        let at = self.at;
        self.at += 1;
        Some(Ok(self.buf[at]))
    }
}

impl MountId {
    /// Shortcut to stat all information of a mount.
    pub fn stat_full(self) -> io::Result<Box<StatMount>> {
        StatMount::stat(self)
    }

    /// Stat a mount id.
    pub fn stat(self, what: StatMountFlags) -> io::Result<Box<StatMount>> {
        StatMount::builder()
            .set_flags(true, what)
            .mount_id(self)
            .stat()
    }

    /// Stat a mount id in a specific namespace.
    pub fn stat_ns(self, what: StatMountFlags, namespace: MountNsId) -> io::Result<Box<StatMount>> {
        StatMount::builder()
            .set_flags(true, what)
            .mount_id(self)
            .mount_namespace(namespace)
            .stat()
    }
}

/// A builder for a `statmount(2)` call.
#[derive(Clone, Copy, Debug)]
pub struct StatMountBuilder {
    flags: StatMountFlags,
    request: MountIdRequest,
}

impl Default for StatMountBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl StatMountBuilder {
    /// A new empty request.
    pub const fn new() -> Self {
        Self {
            flags: StatMountFlags::SB_BASIC,
            request: MountIdRequest::new(),
        }
    }

    /// Set the mount ID to request stats for or begin listing from.
    pub fn mount_id(mut self, id: MountId) -> Self {
        self.request = self.request.mount_id(id);
        self
    }

    /// Set the namespace this request is for. This bumps the kernel requirement from 6.9 to 6.10.
    pub fn mount_namespace(mut self, mnt_ns_id: MountNsId) -> Self {
        self.request = self.request.mount_namespace(mnt_ns_id);
        self
    }

    /// Set/clear flags.
    fn set_flags(mut self, on: bool, flags: StatMountFlags) -> Self {
        if on {
            self.flags |= flags;
        } else {
            self.flags &= !flags;
        }
        self
    }

    /// Request basic superblock information.
    pub fn basic_superblock_info(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::SB_BASIC)
    }

    /// Request basic mount information.
    pub fn basic_mount_info(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::MNT_BASIC)
    }

    /// Request propagation info.
    pub fn propagate_from(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::PROPAGATE_FROM)
    }

    /// Request the root of the mount relative to the root of the file system.
    pub fn mount_root(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::MNT_ROOT)
    }

    /// Request the mount point relative to the current root.
    pub fn mount_point(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::MNT_POINT)
    }

    /// Request the file system type.
    pub fn fs_type(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::FS_TYPE)
    }

    /// Request the mount namespace id.
    pub fn mount_ns_id(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::MNT_NS_ID)
    }

    /// Request the mount option string.
    pub fn mount_options(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::MNT_OPTS)
    }

    /// Request the file system subtype.
    pub fn fs_subtype(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::FS_SUBTYPE)
    }

    /// Request the source (eg. device).
    pub fn source(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::SB_SOURCE)
    }

    /// Request the mount option array.
    pub fn mount_option_array(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::OPT_ARRAY)
    }

    /// Request the mount security option array.
    pub fn mount_security_option_array(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::OPT_SEC_ARRAY)
    }

    /// Request all known options.
    pub fn all(self, on: bool) -> Self {
        self.set_flags(on, StatMountFlags::all())
    }

    /// Perform a `statmount(2)` call.
    pub fn stat(&mut self) -> io::Result<Box<StatMount>> {
        StatMount::request(self)
    }
}

/// Result of a `statmount(2)` call.
#[derive(Debug)]
#[repr(C)]
struct StatMountBase {
    /// Total size, including strings.
    size: u32,
    /// `[str]` Options (comma separated, escaped).
    mnt_opts: u32,
    /// What results were written.
    mask: u64,
    /// Device ID major number.
    sb_dev_major: u32,
    /// Device ID minof number.
    sb_dev_minor: u32,
    /// `..._SUPER_MAGIC`.
    sb_magic: u64,
    /// SB_{RDONLY,SYNCHRONOUS,DIRSYNC,LAZYTIME}.
    sb_flags: SuperblockFlags,
    /// `[str]` Filesystem type.
    fs_type: u32,
    /// Unique ID of mount.
    mnt_id: MountId,
    /// Unique ID of parent (for root == mnt_id).
    mnt_parent_id: MountId,
    /// Reused IDs used in proc/.../mountinfo.
    mnt_id_old: ReusedMountId,
    /// Reused parent IDs used in proc/.../mountinfo.
    mnt_parent_id_old: ReusedMountId,
    /// `MOUNT_ATTR_...`.
    mnt_attr: MountAttr,
    /// `MS_{SHARED,SLAVE,PRIVATE,UNBINDABLE}`
    mnt_propagation: MountPropagation,
    /// ID of shared peer group.
    mnt_peer_group: u64,
    /// Mount receives propagation from this ID.
    mnt_master: u64,
    /// Propagation from in current namespace.
    propagate_from: u64,
    /// `[str]` Root of mount relative to root of fs.
    mnt_root: u32,
    /// `[str]` Mountpoint relative to current root.
    mnt_point: u32,
    /// ID of the mount namespace.
    mnt_ns_id: MountNsId,
    /// `[str]` Subtype of fs_type (if any).
    fs_subtype: u32,
    /// `[str]` Source string of the mount.
    sb_source: u32,
    /// Number of fs options.
    opt_num: u32,
    /// `[str]` Array of nul terminated fs options.
    opt_array: u32,
    /// Number of security options.
    opt_sec_num: u32,
    /// `[str]` Array of nul terminated security options.
    opt_sec_array: u32,
    /// Spare data...
    _spare2: [u64; 46],
}

/// A buffer used for and result of a `statmount(2)` call.
///
/// Since `statmount(2)` can return a lot of data, a struct with the appropriate size for the call
/// will be allocated. This might result in multiple calls to `statmount(2)` until a large enough
/// buffer was allocated.
#[derive(Debug)]
#[repr(C)]
pub struct StatMount {
    /// The fixed data.
    base: StatMountBase,

    /// Variable size part containing strings.
    str: [u8],
}

impl StatMount {
    /// Create a builder for a `statmount(2)` call.
    pub const fn builder() -> StatMountBuilder {
        StatMountBuilder::new()
    }

    /// Shortcut to do a full `statmount(2)` on a mount id.
    pub fn stat(mount_id: MountId) -> io::Result<Box<Self>> {
        Self::builder().all(true).mount_id(mount_id).stat()
    }

    /// Allocate a buffer for a `statmount(2)` call.
    fn with_capacity(size: usize) -> Box<Self> {
        let str_capacity = size - std::mem::size_of::<StatMountBase>();
        let layout =
            std::alloc::Layout::from_size_align(size, std::mem::align_of::<StatMountBase>())
                .expect("bad size for `StatMount::with_capacity()`");
        unsafe {
            let ptr = std::alloc::alloc(layout);
            let intermediate = std::ptr::slice_from_raw_parts_mut(ptr, str_capacity);
            Box::from_raw(intermediate as *mut Self)
        }
    }

    /// Grow a buffer for a `statmount(2)` call.
    fn realloc(self: Box<Self>, size: usize) -> Box<Self> {
        let str_capacity = size - std::mem::size_of::<StatMountBase>();

        let old_capacity = self.str.len();
        let old_size = std::mem::size_of::<StatMountBase>() + old_capacity;
        let old_layout =
            std::alloc::Layout::from_size_align(old_size, std::mem::align_of::<StatMountBase>())
                .expect("bad capacity for `StatMount::realloc()`");

        let this = Box::into_raw(self);
        unsafe {
            let ptr = std::alloc::realloc(this as *mut u8, old_layout, size);
            let intermediate = std::ptr::slice_from_raw_parts_mut(ptr, str_capacity);
            Box::from_raw(intermediate as *mut Self)
        }
    }

    /// Get the thin pointer.
    fn as_mut_raw_ptr(&mut self) -> *mut u8 {
        &raw mut self.base as *mut u8
    }

    /// Perform a `statmount(2)` call.
    pub fn request(req: &mut StatMountBuilder) -> io::Result<Box<Self>> {
        let mut capacity = 32768;
        let mut this = Self::with_capacity(capacity);
        let req_ptr = req.request.finalize(req.flags.bits());
        loop {
            let rc = unsafe {
                libc::syscall(SYS_statmount, req_ptr, this.as_mut_raw_ptr(), capacity, 0)
            };

            if rc == 0 {
                return Ok(this);
            }

            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::EOVERFLOW) || capacity >= 0x1000_0000 {
                return Err(err);
            }

            capacity <<= 1;
            this = Self::realloc(this, capacity);
        }
    }

    fn str(&self) -> &[u8] {
        let len =
            usize::try_from(self.base.size).unwrap() - std::mem::size_of::<StatMountBuilder>();
        &self.str[..len]
    }

    /// Get an option if the mask allows it.
    fn option<T>(&self, flag: StatMountFlags, value: T) -> Option<T> {
        (self.base.mask & flag.bits() == flag.bits()).then_some(value)
    }

    /// Get a string option as a `&CStr` if it is available.
    fn str_slice(&self, flag: StatMountFlags, value: u32) -> Option<&[u8]> {
        let value = self.option(flag, value)?;
        let value = usize::try_from(value).unwrap();
        let str = self.str();
        if value >= str.len() {
            return None;
        }
        Some(&str[value..])
    }

    /// Get a string option as a `&CStr` if it is available.
    fn c_str(&self, flag: StatMountFlags, value: u32) -> Option<&CStr> {
        CStr::from_bytes_until_nul(self.str_slice(flag, value)?).ok()
    }

    /// Get the mount options as a raw C string (if requested).
    ///
    /// This is governed by [`StatMountFlags::MNT_OPTS`].
    pub fn mount_options(&self) -> Option<&CStr> {
        self.c_str(StatMountFlags::MNT_OPTS, self.base.mnt_opts)
    }

    /// Get the device ID.
    ///
    /// This is governed by [`StatMountFlags::SB_BASIC`].
    pub fn device(&self) -> Option<Device> {
        self.option(
            StatMountFlags::SB_BASIC,
            Device {
                major: self.base.sb_dev_major,
                minor: self.base.sb_dev_minor,
            },
        )
    }

    /// Get the super block magic. (`..._SUPER_MAGIC` constant).
    ///
    /// This is a numerical file system id, like `BTRFS_SUPER_MAGIC`.
    ///
    /// This is governed by [`StatMountFlags::SB_BASIC`].
    pub fn superblock_magic(&self) -> Option<u64> {
        self.option(StatMountFlags::SB_BASIC, self.base.sb_magic)
    }

    /// Get the super block flags
    ///
    /// This is governed by [`StatMountFlags::SB_BASIC`].
    pub fn superblock_flags(&self) -> Option<SuperblockFlags> {
        self.option(StatMountFlags::SB_BASIC, self.base.sb_flags)
    }

    /// Get the mount id.
    ///
    /// This is governed by [`StatMountFlags::MNT_BASIC`].
    pub fn id(&self) -> Option<MountId> {
        self.option(StatMountFlags::MNT_BASIC, self.base.mnt_id)
    }

    /// Get the parent mount id.
    ///
    /// This is governed by [`StatMountFlags::MNT_BASIC`].
    pub fn parent_id(&self) -> Option<MountId> {
        self.option(StatMountFlags::MNT_BASIC, self.base.mnt_parent_id)
    }

    /// Get the old [`ReusedMountId`].
    ///
    /// This is governed by [`StatMountFlags::MNT_BASIC`].
    pub fn old_id(&self) -> Option<ReusedMountId> {
        self.option(StatMountFlags::MNT_BASIC, self.base.mnt_id_old)
    }

    /// Get the parent's old [`ReusedMountId`].
    ///
    /// This is governed by [`StatMountFlags::MNT_BASIC`].
    pub fn old_parent_id(&self) -> Option<ReusedMountId> {
        self.option(StatMountFlags::MNT_BASIC, self.base.mnt_parent_id_old)
    }

    /// Get the mount attributes.
    ///
    /// This is governed by [`StatMountFlags::MNT_BASIC`].
    pub fn attr(&self) -> Option<MountAttr> {
        self.option(StatMountFlags::MNT_BASIC, self.base.mnt_attr)
    }

    /// Get the mount propagation flags.
    ///
    /// This is governed by [`StatMountFlags::MNT_BASIC`].
    pub fn propagation(&self) -> Option<MountPropagation> {
        self.option(StatMountFlags::MNT_BASIC, self.base.mnt_propagation)
    }

    /// Get the mount peer group id (or 0 for non-shared mounts).
    ///
    /// This is governed by [`StatMountFlags::MNT_BASIC`].
    pub fn peer_group_id(&self) -> Option<u64> {
        self.option(StatMountFlags::MNT_BASIC, self.base.mnt_peer_group)
    }

    /// Get the mount master group id for slave mounts.
    ///
    /// This is governed by [`StatMountFlags::MNT_BASIC`].
    pub fn master_group_id(&self) -> Option<u64> {
        self.option(StatMountFlags::MNT_BASIC, self.base.mnt_master)
    }

    /// Get the source string of a file system.
    ///
    /// This is governed by [`StatMountFlags::SB_SOURCE`].
    pub fn source(&self) -> Option<&CStr> {
        self.c_str(StatMountFlags::SB_SOURCE, self.base.sb_source)
    }

    /// Get the propagate-from value.
    ///
    /// This is governed by [`StatMountFlags::PROPAGATE_FROM`].
    pub fn propagate_from(&self) -> Option<u64> {
        self.option(StatMountFlags::PROPAGATE_FROM, self.base.propagate_from)
    }

    /// Get the root of the mount relative to the root fs.
    ///
    /// This is governed by [`StatMountFlags::MNT_ROOT`].
    pub fn mount_root(&self) -> Option<&CStr> {
        self.c_str(StatMountFlags::MNT_ROOT, self.base.mnt_root)
    }

    /// Get the mount point.
    ///
    /// This is governed by [`StatMountFlags::MNT_POINT`].
    pub fn mount_point(&self) -> Option<&CStr> {
        self.c_str(StatMountFlags::MNT_POINT, self.base.mnt_point)
    }

    /// Get the file system type.
    ///
    /// This is governed by [`StatMountFlags::FS_TYPE`].
    pub fn fs_type(&self) -> Option<u32> {
        self.option(StatMountFlags::FS_TYPE, self.base.fs_type)
    }

    /// Get the ID of the mount namespace.
    ///
    /// This is governed by [`StatMountFlags::MNT_NS_ID`].
    pub fn mount_namespace_id(&self) -> Option<MountNsId> {
        self.option(StatMountFlags::MNT_NS_ID, self.base.mnt_ns_id)
    }

    /// Get the file system subtype.
    ///
    /// This is governed by [`StatMountFlags::FS_SUBTYPE`].
    pub fn fs_subtype(&self) -> Option<&CStr> {
        self.c_str(StatMountFlags::FS_SUBTYPE, self.base.fs_subtype)
    }

    /// Get an iterator over the separate mount options.
    ///
    /// This is governed by [`StatMountFlags::OPT_ARRAY`].
    pub fn options(&self) -> Option<OptionIter> {
        Some(OptionIter::new(
            usize::try_from(self.base.opt_num).unwrap(),
            self.str_slice(StatMountFlags::OPT_ARRAY, self.base.opt_array)?,
        ))
    }

    /// Get an iterator over the separate security options.
    ///
    /// This is governed by [`StatMountFlags::OPT_SEC_ARRAY`].
    pub fn security_options(&self) -> Option<OptionIter> {
        Some(OptionIter::new(
            usize::try_from(self.base.opt_sec_num).unwrap(),
            self.str_slice(StatMountFlags::OPT_SEC_ARRAY, self.base.opt_sec_array)?,
        ))
    }
}

pub struct OptionIter<'a> {
    remaining: usize,
    buf: &'a [u8],
}

impl<'a> OptionIter<'a> {
    fn new(count: usize, buf: &'a [u8]) -> Self {
        Self {
            remaining: count,
            buf,
        }
    }
}

impl<'a> Iterator for OptionIter<'a> {
    type Item = &'a CStr;

    fn next(&mut self) -> Option<&'a CStr> {
        if self.remaining == 0 {
            return None;
        }

        self.remaining -= 1;

        match self.buf.iter().position(|&b| b == 0) {
            None | Some(0) => {
                self.remaining = 0;
                None
            }
            Some(n) => {
                let out = unsafe { CStr::from_bytes_with_nul_unchecked(&self.buf[..=n]) };
                self.buf = &self.buf[(n + 1)..];
                Some(out)
            }
        }
    }
}
