#![allow(unused_macros)]
#![allow(dead_code)]

pub mod bindings;

pub mod util;
pub use util::error::Error;
pub use util::foreign::CloneToForeign;
pub use util::foreign::FromForeign;
pub use util::foreign::IntoNative;
pub use util::foreign::OwnedPointer;
pub type Result<T> = std::result::Result<T, Error>;
