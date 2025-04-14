//! Some higher-level file system operations found in `std::fs` with file descriptor support, such
//! as `create_dir_all` but with a file descriptor as first parameter.

mod create_path;
pub use create_path::CreatePath;

pub mod read_dir;
#[doc(inline)]
pub use read_dir::{ReadDir, read_dir};

pub mod stat;
#[doc(inline)]
pub use stat::Stat;
