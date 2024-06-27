//! Bindings for the QOM Object class
//!
//! @author Paolo Bonzini

use std::ffi::CStr;

use crate::bindings::object_new;
use crate::bindings::Object;

use crate::qom_isa;

use crate::qom::refs::Arc;
use crate::qom::refs::ObjectCast;

/// Trait exposed by all structs corresponding to QOM objects.
/// Defines "class methods" for the class.  Usually these can be
/// implemented on the class itself; here, using a trait allows
/// each class to define `TYPE`, and it also lets `new()` return the
/// right type.
///
/// # Safety
///
/// - the first field of the struct must be of `Object` type,
///   or derived from it
///
/// - `TYPE` must match the type name used in the `TypeInfo` (no matter
///   if it is defined in C or Rust).
///
/// - the struct must be `#[repr(C)]`
pub unsafe trait ObjectType: Sized {
    const TYPE: &'static CStr;
}

unsafe impl ObjectType for Object {
    const TYPE: &'static CStr = c"object";
}

qom_isa!(Object);
