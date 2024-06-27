//! Bindings for the QOM Device class
//!
//! @author Paolo Bonzini

use crate::qom::object::ObjectType;

use crate::qom::refs::IsA;
use crate::qom::refs::ObjectCast;

use crate::bindings;
use crate::bindings::device_cold_reset;
use crate::bindings::device_realize;
use crate::bindings::DeviceState;
use crate::bindings::Object;

use crate::qom_isa;

use crate::Result;

use std::ffi::CStr;
use std::ops::Deref;
use std::ptr::null_mut;

unsafe impl ObjectType for DeviceState {
    const TYPE: &'static CStr = c"device";
}

qom_isa!(DeviceState, Object);

/// Trait for methods exposed by the Object class.  The methods can be
/// called on all objects that have the trait `IsA<Object>`.
///
/// The trait should only be used through the blanket implementation,
/// which guarantees safety via `IsA`
pub trait DeviceMethods: Deref
where
    Self::Target: IsA<DeviceState>,
{
    fn realize(&self) -> Result<()> {
        let device = self.upcast::<DeviceState>();
        let mut err: *mut bindings::Error = null_mut();
        // SAFETY: safety of this is the requirement for implementing IsA
        unsafe {
            device_realize(device.as_mut_ptr(), &mut err);
            crate::Error::err_or_default(err)
        }
    }

    fn cold_reset(&self) {
        let device = self.upcast::<DeviceState>();
        // SAFETY: safety of this is the requirement for implementing IsA
        unsafe { device_cold_reset(device.as_mut_ptr()) }
    }
}

impl<R: Deref> DeviceMethods for R where R::Target: IsA<DeviceState> {}
