//! Mount point handles.

use std::ffi::{CStr, c_int, c_uint, c_void};
use std::io;
use std::marker::PhantomData;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};

use bitflags::bitflags;

use crate::CPath;
use crate::error::{io_assert, io_bail};
use crate::mount::sys;

#[cfg(feature = "open")]
use crate::open::OpenHow;

pub use super::sys::MountAttr;

bitflags! {
    /// Flags for handling the "from" and "to" parts of a [`move_mount`](Mount::move_mount())
    /// operation.
    pub struct MoveMount: c_uint {
        /// Follow symlinks on the "from" path.
        const F_SYMLINKS    = 0x0000_0001;
        /// Trigger auto-mounts on the "from" path.
        const F_AUTOMOUNTS  = 0x0000_0002;
        /// Permit an empty "from" path to use the "from" file descriptor directly.
        const F_EMPTY_PATH  = 0x0000_0004;

        /// Mask of valid flags for the "from" side.
        const F_MASK =
            Self::F_SYMLINKS.bits() | Self::F_AUTOMOUNTS.bits() | Self::F_EMPTY_PATH.bits();

        /// Follow symlinks on the "to" path.
        const T_SYMLINKS    = 0x0000_0010;
        /// Trigger auto-mounts on the "to" path.
        const T_AUTOMOUNTS  = 0x0000_0020;
        /// Permit an empty "to" path to use the "to" file descriptor directly.
        const T_EMPTY_PATH  = 0x0000_0040;

        /// Mask of valid flags for the "to" side.
        const T_MASK =
            Self::T_SYMLINKS.bits() | Self::T_AUTOMOUNTS.bits() | Self::T_EMPTY_PATH.bits();

        /// Set the sharing group instead of moving a mount point.
        const SET_GROUP = 0x0000_0100;

        /// Mount *beneath* a top mount.
        const BENEATH = 0x0000_0200;
    }
}

bitflags! {
    /// Flags for the [`open_tree`](Mount::open_tree()) function.
    pub struct OpenTree: c_uint {
        // these are OPEN_TREE_* flags

        /// Clone the subtree at this point. This can be used to create bind mounts.
        /// Without this flag, `open_tree` behaves like `open` with `O_PATH`.
        const CLONE   = 0x0000_0001;

        /// Set the close-on-exec flag on the resulting file descriptor.
        const CLOEXEC = 0o0200_0000; // octal!

        /// Clone the tree recursively.
        ///
        /// The value of this is the same as `AT_RECURSIVE`.
        const RECURSIVE = libc::AT_RECURSIVE as c_uint;
    }
}

/// The raw data we can use without lifetimes.
#[derive(Clone, Debug)]
#[repr(C)]
struct RawSetAttr {
    attr_set: u64,
    attr_clr: u64,
    propagation: u64,
    userns_fd: u64,
}

/// The `struct mount_attr`
#[derive(Clone, Debug)]
#[repr(C)]
pub struct MountSetAttr<'a> {
    attr: RawSetAttr,
    _fd_lifetime: PhantomData<&'a ()>,
}

impl Default for MountSetAttr<'static> {
    fn default() -> Self {
        Self {
            attr: RawSetAttr {
                attr_set: 0,
                attr_clr: 0,
                propagation: 0,
                userns_fd: 0,
            },
            _fd_lifetime: PhantomData,
        }
    }
}

impl MountSetAttr<'_> {
    /// Create a new empty instance.
    pub fn new() -> MountSetAttr<'static> {
        Default::default()
    }

    /// Set mount attributes.
    pub fn set(mut self, attr: MountAttr) -> Self {
        let attr = u64::from(attr.bits());
        self.attr.attr_set |= attr;
        self.attr.attr_clr &= !attr;
        self
    }

    /// Clear mount attributes.
    pub fn clear(mut self, attr: MountAttr) -> Self {
        let attr = u64::from(attr.bits());
        self.attr.attr_set &= !attr;
        self.attr.attr_clr |= attr;
        self
    }

    /// Remove flags previously added via `clear` or `set`.
    pub fn keep(mut self, attr: MountAttr) -> Self {
        let attr = u64::from(attr.bits());
        self.attr.attr_set &= !attr;
        self.attr.attr_clr &= !attr;
        self
    }

    /// Set the idmap file descriptor.
    pub fn idmap<'new, T: AsRawFd + ?Sized + 'new>(self, fd: &'new T) -> MountSetAttr<'new> {
        MountSetAttr::<'new> {
            attr: RawSetAttr {
                userns_fd: fd.as_raw_fd() as u64,
                attr_set: self.attr.attr_set | u64::from(MountAttr::IDMAP.bits()),
                attr_clr: self.attr.attr_set & !u64::from(MountAttr::IDMAP.bits()),
                ..self.attr
            },
            _fd_lifetime: PhantomData,
        }
    }

    /// Set the idmap user namespace file descriptor.
    ///
    /// # Safety
    ///
    /// It is up to the user to make sure the file descriptor remains valid until it is used for a
    /// `mount_setattr` call.
    pub unsafe fn idmap_fd(self, userns_fd: RawFd) -> MountSetAttr<'static> {
        MountSetAttr {
            attr: RawSetAttr {
                userns_fd: userns_fd as u64,
                attr_set: self.attr.attr_set | u64::from(MountAttr::IDMAP.bits()),
                attr_clr: self.attr.attr_set & !u64::from(MountAttr::IDMAP.bits()),
                ..self.attr
            },
            _fd_lifetime: PhantomData,
        }
    }

    /// An `MS_` flag to set the propagation to. `0` leaves it unchagned.
    pub fn propagation(mut self, propagation: u64) -> Self {
        self.attr.propagation = propagation;
        self
    }
}

/// Handle to a mount point. Used to move or bind mount points or change their attributes.
pub struct Mount {
    pub(crate) fd: OwnedFd,
}

impl AsFd for Mount {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

impl AsRawFd for Mount {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl IntoRawFd for Mount {
    fn into_raw_fd(self) -> RawFd {
        self.fd.into_raw_fd()
    }
}

impl FromRawFd for Mount {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        unsafe {
            Self {
                fd: OwnedFd::from_raw_fd(fd),
            }
        }
    }
}

impl Mount {
    /// Open a mount tree from a given path.
    ///
    /// Equivalent to calling [`open_tree_at`](Self::open_tree_at()) with `AT_FDCWD` as directory
    /// file descriptor.
    pub fn open_tree<P>(path: &P, flags: OpenTree, at_flags: c_uint) -> io::Result<Self>
    where
        P: ?Sized + CPath,
    {
        path.c_path(move |path| Self::open_tree_at_raw(libc::AT_FDCWD, path, flags, at_flags))?
    }

    /// Open a mount tree from a given path relative to a directory file descriptor `dfd`.
    ///
    /// If `flags` contains [`OpenTree::CLONE`], this creates a file handle to a separate "bind"
    /// mount which can later be installed via the [`move_mount`][move] method family,
    /// and therefore `path` may point to a subdirectory, rather than an actual mount point.
    /// Without the flag, [`move_mount`][move] will literally move the mount point.
    ///
    /// [move]: Self::move_mount()
    pub fn open_tree_at<D, P>(
        dfd: &D,
        path: &P,
        flags: OpenTree,
        at_flags: c_uint,
    ) -> io::Result<Self>
    where
        D: ?Sized + AsRawFd,
        P: ?Sized + CPath,
    {
        let dfd = dfd.as_raw_fd();
        path.c_path(move |path| Self::open_tree_at_raw(dfd, path, flags, at_flags))?
    }

    /// Perform the `open_tree` call directly without any additional allocations happening.
    /// Useful for `vfork`/`CLONE_VM` situations.
    ///
    /// Returns `true` on success, `false` on failure.
    pub fn open_tree_at_raw(
        dfd: RawFd,
        path: &CStr,
        flags: OpenTree,
        at_flags: c_uint,
    ) -> io::Result<Self> {
        let rc = unsafe { Self::open_tree_at_raw_do(dfd, path, flags, at_flags) };
        io_assert!(rc >= 0);
        let fd = unsafe { OwnedFd::from_raw_fd(rc as RawFd) };
        Ok(Self { fd })
    }

    /// Perform the `open_tree` call directly without any additional allocations happening.
    /// Useful for `vfork`/`CLONE_VM` situations.
    ///
    /// Returns the file descriptor on success, otherwise returns `-1` and `errno` is set.
    ///
    /// # Safety
    ///
    /// It is up to the caller to know this is safe.
    pub unsafe fn open_tree_at_raw_do(
        dfd: RawFd,
        path: &CStr,
        flags: OpenTree,
        at_flags: c_uint,
    ) -> RawFd {
        unsafe {
            libc::syscall(
                sys::SYS_open_tree,
                dfd,
                path.as_ptr(),
                flags.bits() | at_flags,
            ) as RawFd
        }
    }

    /// Move this mount point to a new location.
    pub fn move_mount<P>(&self, dest: &P, move_flags: MoveMount) -> io::Result<()>
    where
        P: ?Sized + CPath,
    {
        dest.c_path(move |dest| self.move_mount_at_raw(libc::AT_FDCWD, dest, move_flags))?
    }

    /// Move this mount point to a new location, relative to a directory file descriptor.
    pub fn move_mount_at<D, P>(&self, dfd: &D, dest: &P, move_flags: MoveMount) -> io::Result<()>
    where
        D: ?Sized + AsRawFd,
        P: ?Sized + CPath,
    {
        let dfd = dfd.as_raw_fd();
        dest.c_path(move |dest| self.move_mount_at_raw(dfd, dest, move_flags))?
    }

    /// Perform the move, raw parameters.
    pub fn move_mount_at_raw(
        &self,
        dfd: RawFd,
        dest: &CStr,
        move_flags: MoveMount,
    ) -> io::Result<()> {
        if move_flags.intersects(MoveMount::F_MASK) {
            io_bail!("must not use source flags in move_mount()");
        }
        io_assert!(unsafe { self.move_mount_at_raw_do(dfd, dest, move_flags) });
        Ok(())
    }

    /// Perform the `move_mount` call directly without any additional allocations happening.
    /// Useful for `vfork`/`CLONE_VM` situations.
    ///
    /// Returns `true` on success, `false` on failure.
    ///
    /// # Safety
    ///
    /// `move_flags` are NOT verified!
    pub unsafe fn move_mount_at_raw_do(
        &self,
        dfd: RawFd,
        dest: &CStr,
        move_flags: MoveMount,
    ) -> bool {
        let move_flags = move_flags | MoveMount::F_EMPTY_PATH;

        0 == unsafe {
            libc::syscall(
                sys::SYS_move_mount,
                self.fd.as_raw_fd(),
                b"\0",
                dfd,
                dest.as_ptr(),
                move_flags.bits(),
            )
        }
    }

    /// Change attributes of the this mount point.
    pub fn setattr(&self, attr: &MountSetAttr, at_flags: c_int) -> io::Result<()> {
        let rc = unsafe {
            libc::syscall(
                sys::SYS_mount_setattr,
                self.fd.as_raw_fd(),
                b"\0",
                libc::AT_EMPTY_PATH | at_flags,
                attr,
                std::mem::size_of_val(attr),
            )
        };
        io_assert!(rc == 0);
        Ok(())
    }

    /// Open something inside this mount point.
    ///
    /// This implies setting `RESOLVE_IN_ROOT` and using this file descriptor as root file system.
    #[cfg(feature = "open")]
    pub fn open<P>(&self, how: OpenHow, path: &P) -> io::Result<OwnedFd>
    where
        P: ?Sized + CPath,
    {
        how.resolve_in_root(true).at_fd(self).open(path)
    }

    /// Open a file inside this mount point.
    ///
    /// This implies setting `RESOLVE_IN_ROOT` and using this file descriptor as root file system.
    #[cfg(feature = "open")]
    pub fn open_file<P>(&self, how: OpenHow, path: &P) -> io::Result<std::fs::File>
    where
        P: ?Sized + CPath,
    {
        how.resolve_in_root(true).at_fd(self).open_file(path)
    }

    /// Read the contents of a file in this mount point.
    #[cfg(feature = "open")]
    pub fn read<P>(&self, path: &P) -> io::Result<Vec<u8>>
    where
        P: ?Sized + CPath,
    {
        use std::io::Read;

        let mut file = self.open_file(OpenHow::new_read(), path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    }

    /// This is an unsafe way to force-open a subtree via `open_tree`.
    ///
    /// This first spawns a subprocess in a new slave-mount-namespace, mounts the parent mount at
    /// `/` to create a "path" to the subdirectory, then calls `open_tree`.
    ///
    /// # Safety
    ///
    /// This is highly experimental and involves a `clone(2)` call with shared memory and file
    /// descriptor tables.
    pub unsafe fn open_subtree<P>(&self, path: &P) -> io::Result<Self>
    where
        P: ?Sized + CPath,
    {
        path.c_path(move |path| open_subtree(self, path))?
    }
}

struct Shared<'a> {
    tree_fd: RawFd,
    errno: c_int,
    mount: &'a Mount,
    subdir: &'a CStr,
}

fn open_subtree(mount: &Mount, subdir: &CStr) -> io::Result<Mount> {
    let mut shared = Box::new(Shared {
        tree_fd: -1,
        errno: libc::ENOSYS,
        mount,
        subdir,
    });
    let mut pid_fd: c_int = -1;

    let pid = unsafe {
        const STACK_SIZE: usize = 64 * 1024;
        let stack = std::alloc::alloc(std::alloc::Layout::array::<u8>(STACK_SIZE).unwrap());
        let stack = Box::from_raw(std::ptr::slice_from_raw_parts_mut(stack, STACK_SIZE));
        libc::clone(
            open_subtree_process,
            stack.as_ptr().add(STACK_SIZE) as *mut c_void,
            libc::CLONE_NEWNS
                | libc::CLONE_FILES
                | libc::CLONE_VM
                | libc::CLONE_PIDFD
                | libc::SIGCHLD,
            &raw mut *shared as *mut c_void,
            &raw mut pid_fd as *mut libc::pid_t,
        )
    };

    if pid < 0 {
        return Err(io::Error::last_os_error());
    }
    let pid_fd = unsafe { OwnedFd::from_raw_fd(pid_fd) };

    loop {
        let rc = unsafe { libc::waitpid(pid, std::ptr::null_mut(), 0) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    pid_fd.as_raw_fd(),
                    libc::SIGKILL,
                    std::ptr::null::<c_void>(),
                    0,
                )
            };

            return Err(io::Error::last_os_error());
        }
        if rc == pid {
            break;
        }
    }

    let tree_fd = unsafe { std::ptr::read_volatile(&raw const shared.tree_fd) };
    if tree_fd < 0 {
        return Err(io::Error::from_raw_os_error(shared.errno));
    }
    Ok(unsafe { Mount::from_raw_fd(tree_fd) })
}

extern "C" fn open_subtree_process(shared: *mut c_void) -> c_int {
    let shared = unsafe { &mut *(shared as *mut Shared<'static>) };

    unsafe {
        let rc = libc::mount(
            std::ptr::null(),
            c"/".as_ptr(),
            std::ptr::null(),
            libc::MS_REC | libc::MS_SLAVE,
            std::ptr::null(),
        );
        if rc != 0 {
            shared.errno = *libc::__errno_location();
            return 1;
        }
        if !shared
            .mount
            .move_mount_at_raw_do(-1, c"/", MoveMount::empty())
        {
            shared.errno = *libc::__errno_location();
            return 1;
        }
        let rc = libc::fchdir(shared.mount.as_raw_fd());
        if rc != 0 {
            shared.errno = *libc::__errno_location();
            return 1;
        }
        let rc = libc::chroot(c".".as_ptr());
        if rc != 0 {
            shared.errno = *libc::__errno_location();
            return 1;
        }
        shared.tree_fd = Mount::open_tree_at_raw_do(
            shared.mount.as_raw_fd(),
            shared.subdir,
            OpenTree::CLOEXEC | OpenTree::CLONE,
            0,
        );
        shared.errno = *libc::__errno_location();
    }

    0
}
