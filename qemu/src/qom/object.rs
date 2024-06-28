//! Bindings for the QOM Object class
//!
//! @author Paolo Bonzini

use std::borrow::Cow;
use std::ffi::CStr;
use std::fmt;
use std::ops::Deref;

use cstr::cstr;

use crate::bindings::object_get_typename;
use crate::bindings::object_new;
use crate::bindings::object_unparent;
use crate::bindings::Object;

use crate::qom_isa;

use crate::qom::refs::IsA;
use crate::qom::refs::ObjectCast;
use crate::qom::refs::Owned;

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
    const TYPE: &'static CStr = cstr!("object");
}

// ------------------------------
// Object class

qom_isa!(Object);

/// Trait for class methods exposed by the Object class.  The methods can be
/// called on all objects that have the trait `IsA<Object>`.
///
/// The trait should only be used through the blanket implementation,
/// which guarantees safety via `IsA`

pub trait ObjectClassMethods: IsA<Object> {
    /// Return a new reference counted instance of this class
    fn new() -> Owned<Self> {
        // SAFETY: the object created by object_new is allocated on
        // the heap and has a reference count of 1
        unsafe {
            let obj = &*object_new(Self::TYPE.as_ptr());
            Owned::from_raw(obj.unsafe_cast::<Self>())
        }
    }
}

/// Trait for methods exposed by the Object class.  The methods can be
/// called on all objects that have the trait `IsA<Object>`.
///
/// The trait should only be used through the blanket implementation,
/// which guarantees safety via `IsA`
pub trait ObjectMethods: Deref
where
    Self::Target: IsA<Object>,
{
    /// Return the name of the type of `self`
    fn typename(&self) -> Cow<'_, str> {
        let obj = self.upcast::<Object>();
        // SAFETY: safety of this is the requirement for implementing IsA
        // The result of the C API has static lifetime
        let type_cstr = unsafe {
            let type_cstr = object_get_typename(obj.as_mut_ptr());
            CStr::from_ptr(type_cstr)
        };

        type_cstr.to_string_lossy()
    }

    /// Remove the object from the QOM tree
    fn unparent(&self) {
        let obj = self.upcast::<Object>();
        // SAFETY: safety of this is the requirement for implementing IsA
        unsafe {
            object_unparent(obj.as_mut_ptr());
        }
    }

    /// Convenience function for implementing the Debug trait
    fn debug_fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple(&self.typename())
            .field(&(self as *const Self))
            .finish()
    }
}

impl<R> ObjectClassMethods for R where R: IsA<Object> {}
impl<R: Deref> ObjectMethods for R where R::Target: IsA<Object> {}
