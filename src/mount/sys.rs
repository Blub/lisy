//! Constants for the fsmount API.
//!
//! The fsmount related syscalls use asm-generic, so they are the same across architectures.

use bitflags::bitflags;

#[allow(non_upper_case_globals)]
#[rustfmt::skip]
mod syscalls {
    use std::os::raw::c_long;

    /// open_tree(2)
    pub const SYS_open_tree     : c_long = 428;

    /// move_mount(2)
    pub const SYS_move_mount    : c_long = 429;

    /// fsopen(2)
    pub const SYS_fsopen        : c_long = 430;

    /// fsconfig(2)
    pub const SYS_fsconfig      : c_long = 431;

    /// fsmount(2)
    pub const SYS_fsmount       : c_long = 432;

    /// fspick(2)
    pub const SYS_fspick        : c_long = 433;

    /// mount_setattr(2)
    pub const SYS_mount_setattr : c_long = 442;

    /// statmount(2)
    pub const SYS_statmount     : c_long = 457;

    /// listmount(2)
    pub const SYS_listmount     : c_long = 458;
}
pub use syscalls::*;

bitflags! {
    /// Mount attributes for `Superblock::mount` or Mount::setattr.
    #[derive(Clone, Copy, Debug)]
    pub struct MountAttr: std::os::raw::c_uint {
        /// Read-only flag.
        const RDONLY      = 0x0000_0001;

        /// Disable `suid` executables.
        const NOSUID      = 0x0000_0002;

        /// Disable device special files.
        const NODEV       = 0x0000_0004;

        /// Disallow mapping files as executable.
        const NOEXEC      = 0x0000_0008;

        /// Set the `relatime` mount option.
        const RELATIME    = 0x0000_0000;

        /// Set the `noatime` mount option.
        const NOATIME     = 0x0000_0010;

        /// Set the `strictatime` mount option.
        const STRICTATIME = 0x0000_0020;

        /// Set the `nodiratime` mount option.
        const NODIRATIME  = 0x0000_0080;

        /// Perform user id mapping on the mount.
        const IDMAP       = 0x0010_0000;

        /// Disable symlinks on the mount.
        const NOSYMFOLLOW = 0x0020_0000;
    }
}

bitflags! {
    /// Request mask for `statmount(2)`.
    #[derive(Clone, Copy, Debug)]
    pub struct StatMountFlags: u64 {
        /// Request basic superblock information.
        const SB_BASIC       = 0x00000001;

        /// Request basic mount information.
        const MNT_BASIC      = 0x00000002;

        /// Request propagate information.
        const PROPAGATE_FROM = 0x00000004;

        /// Want/got mnt_root
        const MNT_ROOT       = 0x00000008;

        /// Want/got mnt_point
        const MNT_POINT      = 0x00000010;

        /// Want/got fs_type
        const FS_TYPE        = 0x00000020;

        /// Want/got mnt_ns_id
        const MNT_NS_ID      = 0x00000040;

        /// Want/got mnt_opts
        const MNT_OPTS       = 0x00000080;

        /// Want/got fs_subtype
        const FS_SUBTYPE     = 0x00000100;

        /// Want/got sb_source
        const SB_SOURCE      = 0x00000200;

        /// Want/got opt_...
        const OPT_ARRAY      = 0x00000400;

        /// Want/got opt_sec...
        const OPT_SEC_ARRAY  = 0x00000800;
    }
}

bitflags! {
    /// The superblock flags exposed by `statmount(2)`.
    #[derive(Clone, Copy, Debug)]
    pub struct SuperblockFlags: u32 {
        /// Mount read-only.
        const RDONLY       = 1 << 0;
        /// Writes are synced at once.
        const SYNCHRONOUS  = 1 << 4;
        /// Directory modifications are synchronous.
        const DIRSYNC      = 1 << 7;
        /// Update the on-disk [acm]times lazily.
        const LAZYTIME     = 1 << 25;
    }
}

bitflags! {
    /// Mount propagation flags.
    #[derive(Clone, Copy, Debug)]
    pub struct MountPropagation: u64 {
        /// An unbindable mount.
        const UNBINDABLE = 1<<17;
        /// A private mount.
        const PRIVATE    = 1<<18;
        /// A slave mount.
        const SLAVE      = 1<<19;
        /// A shjared mount.
        const SHARED     = 1<<20;
    }
}
