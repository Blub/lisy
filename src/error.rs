//! Common error handling stuff

#![allow(unused_macros)]
#![allow(unused_imports)]

/// Like failure's `format_err` but producing a `std::io::Error`.
macro_rules! io_format_err {
    ($($msg:tt)+) => {
        ::std::io::Error::new(::std::io::ErrorKind::Other, format!($($msg)+))
    };
}
pub(crate) use io_format_err;

/// Like failure's `bail` but producing a `std::io::Error`.
macro_rules! io_bail {
    ($($msg:tt)+) => {{
        return Err($crate::error::io_format_err!($($msg)+));
    }};
}
pub(crate) use io_bail;

/// Shortcut to return an `io::Error::last_os_error`.
///
/// This is effectively `return Err(::std::io::Error::last_os_error().into());`.
macro_rules! io_bail_last {
    () => {
        return Err(::std::io::Error::last_os_error().into());
    };
}
pub(crate) use io_bail_last;

/// Non-panicking assertion: shortcut for returning an `io::Error` if the condition is not met.
/// Essentially: `if !expr { io_bail_last!() }`.
macro_rules! io_assert {
    ($value:expr) => {
        if !$value {
            $crate::error::io_bail_last!();
        }
    };
}
pub(crate) use io_assert;
