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
}
pub use syscalls::*;

bitflags! {
    /// Mount attributes for `Superblock::mount` or Mount::setattr.
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
    pub struct StatMount: std::os::raw::c_uint {
        /// Want/got sb_...
        const STATMOUNT_SB_BASIC       = 0x00000001;
        /// Want/got mnt_...
        const STATMOUNT_MNT_BASIC      = 0x00000002;
        /// Want/got propagate_from
        const STATMOUNT_PROPAGATE_FROM = 0x00000004;
        /// Want/got mnt_root
        const STATMOUNT_MNT_ROOT       = 0x00000008;
        /// Want/got mnt_point
        const STATMOUNT_MNT_POINT      = 0x00000010;
        /// Want/got fs_type
        const STATMOUNT_FS_TYPE	       = 0x00000020;
        /// Want/got mnt_ns_id
        const STATMOUNT_MNT_NS_ID      = 0x00000040;
        /// Want/got mnt_opts
        const STATMOUNT_MNT_OPTS       = 0x00000080;
        /// Want/got fs_subtype
        const STATMOUNT_FS_SUBTYPE     = 0x00000100;
        /// Want/got sb_source
        const STATMOUNT_SB_SOURCE      = 0x00000200;
        /// Want/got opt_...
        const STATMOUNT_OPT_ARRAY      = 0x00000400;
        /// Want/got opt_sec...
        const STATMOUNT_OPT_SEC_ARRAY  = 0x00000800;
    }
}
