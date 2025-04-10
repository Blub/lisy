use std::ffi::{CStr, CString, OsStr, OsString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::error::io_format_err;

#[allow(dead_code)]
#[inline]
pub(crate) fn io_c_string<T: Into<Vec<u8>>>(t: T) -> io::Result<CString> {
    CString::new(t).map_err(|_| io_format_err!("null byte in path"))
}

#[allow(dead_code)]
#[inline]
pub(crate) fn io_c_os_str(s: &OsStr) -> io::Result<CString> {
    io_c_string(s.as_bytes())
}

/// Path helper for minimal copying when generating C-Strings.
///
/// This maps [`NulError`](std::ffi::NulError) to `io::ErrorKind::Unknown`.
pub trait CPath {
    /// Call `func` with a `CStr` version of `self`.
    fn c_path<R, F>(&self, func: F) -> io::Result<R>
    where
        F: FnOnce(&CStr) -> R;
}

impl CPath for PathBuf {
    fn c_path<R, F>(&self, func: F) -> io::Result<R>
    where
        F: FnOnce(&CStr) -> R,
    {
        self.as_os_str().c_path(func)
    }
}

impl CPath for Path {
    fn c_path<R, F>(&self, func: F) -> io::Result<R>
    where
        F: FnOnce(&CStr) -> R,
    {
        self.as_os_str().c_path(func)
    }
}

impl CPath for OsString {
    fn c_path<R, F>(&self, func: F) -> io::Result<R>
    where
        F: FnOnce(&CStr) -> R,
    {
        self.as_os_str().c_path(func)
    }
}

impl CPath for OsStr {
    fn c_path<R, F>(&self, func: F) -> io::Result<R>
    where
        F: FnOnce(&CStr) -> R,
    {
        Ok(func(&io_c_string(self.as_bytes())?))
    }
}

impl CPath for String {
    fn c_path<R, F>(&self, func: F) -> io::Result<R>
    where
        F: FnOnce(&CStr) -> R,
    {
        AsRef::<OsStr>::as_ref(self).c_path(func)
    }
}

impl CPath for str {
    fn c_path<R, F>(&self, func: F) -> io::Result<R>
    where
        F: FnOnce(&CStr) -> R,
    {
        AsRef::<OsStr>::as_ref(self).c_path(func)
    }
}

impl CPath for CStr {
    fn c_path<R, F>(&self, func: F) -> io::Result<R>
    where
        F: FnOnce(&CStr) -> R,
    {
        Ok(func(self))
    }
}

impl CPath for CString {
    fn c_path<R, F>(&self, func: F) -> io::Result<R>
    where
        F: FnOnce(&CStr) -> R,
    {
        Ok(func(self))
    }
}
