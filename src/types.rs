//! Basic system types such as a device id (major/minor pair).

// This might not be a good module name...
// But a "device id" is quite a "basic" thing at the system level...

/// A device id.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Device {
    /// The major number.
    pub major: u32,
    /// The minor number.
    pub minor: u32,
}
