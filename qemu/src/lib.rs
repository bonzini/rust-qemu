#![allow(unused_macros)]
#![allow(dead_code)]

pub mod bindings;
pub use bindings::DeviceState;
pub use bindings::Object;
pub use bindings::TypeInfo;

pub mod hw;
pub use hw::core::device::DeviceMethods;

pub mod qom;
pub use qom::object::ObjectClassMethods;
pub use qom::object::ObjectMethods;
pub use qom::object::ObjectType;
pub use qom::object_impl::ObjectImpl;
pub use qom::object_impl::TypeImpl;
pub use qom::refs::Arc;
pub use qom::refs::ObjectCast;

pub mod util;
pub use util::error::Error;
pub use util::foreign::CloneToForeign;
pub use util::foreign::FromForeign;
pub use util::foreign::IntoNative;
pub use util::foreign::OwnedPointer;
pub use util::zeroed::Zeroed;
pub type Result<T> = std::result::Result<T, Error>;
