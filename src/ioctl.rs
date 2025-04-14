//! ioctl helpers

use std::ffi::c_int;

pub const IOC_NONE: c_int = 0;
//pub const IOC_WRITE: c_int = 1;
pub const IOC_READ: c_int = 2;

pub const IOC_NRBITS: c_int = 8;
pub const IOC_TYPEBITS: c_int = 8;
pub const IOC_SIZEBITS: c_int = 14;
//pub const IOC_DIRBITS: c_int = 2;

pub const IOC_NRSHIFT: c_int = 0;
pub const IOC_TYPESHIFT: c_int = IOC_NRSHIFT + IOC_NRBITS;
pub const IOC_SIZESHIFT: c_int = IOC_TYPESHIFT + IOC_TYPEBITS;
pub const IOC_DIRSHIFT: c_int = IOC_SIZESHIFT + IOC_SIZEBITS;

pub const fn ioc(dir: c_int, ty: c_int, nr: c_int, size: c_int) -> c_int {
    (dir << IOC_DIRSHIFT) | (ty << IOC_TYPESHIFT) | (nr << IOC_NRSHIFT) | (size << IOC_SIZESHIFT)
}

pub const fn ior<T: Sized>(ty: c_int, nr: c_int) -> c_int {
    ioc(IOC_READ, ty, nr, std::mem::size_of::<T>() as c_int)
}

pub const fn io(ty: c_int, nr: c_int) -> c_int {
    ioc(IOC_NONE, ty, nr, 0)
}
