//! Macros and traits to implement subclasses of Device in Rust
//!
//! @author Paolo Bonzini

#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;

use crate::bindings;
use crate::bindings::DeviceClass;
use crate::bindings::DeviceState;
use crate::bindings::Property;

use crate::qom::object_impl::ObjectImpl;
use crate::qom::object_impl::TypeImpl;

use crate::qom::refs::ObjectCast;

use crate::util::error::Error;

/// Information on which superclass methods are overridden
/// by a Rust-implemented subclass of Device.
pub trait DeviceImpl: ObjectImpl + DeviceTypeImpl {
    /// If not `None`, a function that implements the `realize` member
    /// of the QOM `DeviceClass`.
    const REALIZE: Option<fn(obj: &Self) -> crate::Result<()>> = None;

    /// If not `None`, a function that implements the `unrealize` member
    /// of the QOM `DeviceClass`.
    const UNREALIZE: Option<fn(obj: &Self)> = None;

    /// If not `None`, a function that implements the `cold_reset` member
    /// of the QOM `DeviceClass`.
    const COLD_RESET: Option<fn(obj: &Self)> = None;
}

impl DeviceClass {
    pub fn class_init<T: DeviceImpl>(&mut self) {
        unsafe extern "C" fn rust_cold_reset<T: DeviceImpl>(obj: *mut DeviceState) {
            let f = T::COLD_RESET.unwrap();
            f((&*obj).unsafe_cast::<T>())
        }
        self.cold_reset = T::COLD_RESET.map(|_| rust_cold_reset::<T> as _);

        unsafe extern "C" fn rust_realize<T: DeviceImpl>(
            obj: *mut DeviceState,
            errp: *mut *mut bindings::Error,
        ) {
            let f = T::REALIZE.unwrap();
            let result = f((&*obj).unsafe_cast::<T>());
            Error::ok_or_propagate(result, errp);
        }
        self.realize = T::REALIZE.map(|_| rust_realize::<T> as _);

        unsafe extern "C" fn rust_unrealize<T: DeviceImpl>(obj: *mut DeviceState) {
            let f = T::UNREALIZE.unwrap();
            f((&*obj).unsafe_cast::<T>())
        }
        self.unrealize = T::UNREALIZE.map(|_| rust_unrealize::<T> as _);

        self.properties = <T as DeviceTypeImpl>::properties();

        // Now initialize the ObjectClass from the ObjectImpl.
        self.oc.class_init::<T>();
    }
}

impl DeviceState {
    pub unsafe extern "C" fn rust_class_init<T: DeviceImpl>(
        klass: *mut c_void,
        _data: *mut c_void,
    ) {
        let dc: &mut DeviceClass = &mut *(klass.cast());
        dc.class_init::<T>();
    }
}

/// Internal information on a Rust-implemented subclass of Device.
/// Only public because it is used by macros.
pub unsafe trait DeviceTypeImpl: TypeImpl {
    const CONF_OFFSET: usize;

    // This needs to be here, and not in DeviceImpl, because properties
    // reference statics (for globals defined in C, e.g. qdev_prop_bool)
    // which is unstable (see https://github.com/rust-lang/rust/issues/119618,
    // feature const_refs_to_static)
    fn properties() -> *const Property;
}

pub struct QdevPropBool;
impl QdevPropBool {
    pub const fn convert(value: &bool) -> u64 {
        *value as u64
    }
}

#[macro_export]
macro_rules! qdev_prop {
    (@internal bool, $name:expr, $default:expr, $offset:expr) => {
        $crate::Property {
            name: $name.as_ptr(),
            offset: $offset,
            default: $crate::hw::core::device_impl::QdevPropBool::convert(&($default)),
            info: unsafe { &$crate::bindings::qdev_prop_bool },
        }
    };

    // Replace field with typechecking expression and offset
    ($kind:tt, $name:expr, $type:ty, $default:expr, $field:ident) => {
        qdev_prop!(@internal
            $kind,
            $name,
            (<$crate::conf_type!($type) as ConstDefault>::DEFAULT).$field,
            <$type as $crate::DeviceTypeImpl>::CONF_OFFSET + std::mem::offset_of!($crate::conf_type!($type), $field)
        )
    };
}

#[macro_export]
macro_rules! qdev_define_type {
    ($name:expr, $struct:ident, $conf_ty:ty, $state_ty:ty;
     @extends $super:ty $(,$supers:ty)*;
     @properties [$($props: expr),+]) => {
        $crate::qom_define_type!(
            $name, $struct, $conf_ty, $state_ty;
            @extends $super $(,$supers)*, $crate::Object);

        unsafe impl $crate::DeviceTypeImpl for $struct {
            const CONF_OFFSET: usize = std::mem::offset_of!($struct, conf);

            fn properties() -> *const $crate::Property {
                static mut PROPERTIES: &'static [$crate::Property] = &[$($props),+];

                // SAFETY: The only reference is created here; mut is needed to refer to
                // &qdev_prop_xxx.
                unsafe { PROPERTIES.as_ptr() }
            }
        }
    }
}
