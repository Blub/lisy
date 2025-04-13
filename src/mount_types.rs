//! Types shared between `mount` and `fs` modules.

/// The mount namespace ID.
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct MountNsId(u64);

impl MountNsId {
    /// Get the raw id.
    pub const fn as_raw(self) -> u64 {
        self.0
    }

    /// Create a mount namespace id.
    pub const fn from_raw(id: u64) -> Self {
        Self(id)
    }
}

/// A *unique* mount ID as used in `statmount(2)`, `listmount(2)` or `statx(2)`.
///
/// These are *NOT* the same as ones in `/proc/*/mountinfo`, those would be [`ReusedMountId`]s.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct MountId(u64);

impl MountId {
    /// Get the raw id.
    pub const fn as_raw_id(self) -> u64 {
        self.0
    }

    /// Construct a [`MountId`] from a raw id.
    pub const fn from_raw(id: u64) -> Self {
        Self(id)
    }
}

/// *Reused* mount IDs are the ones used in `/proc/*/mountinfo`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct ReusedMountId(u32);

impl ReusedMountId {
    /// Get the raw id.
    pub const fn as_raw_id(self) -> u32 {
        self.0
    }

    /// Construct a [`ReusedMountId`] from a raw id.
    pub const fn from_raw(id: u32) -> Self {
        Self(id)
    }
}
