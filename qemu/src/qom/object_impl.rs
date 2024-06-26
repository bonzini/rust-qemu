//! Macros and traits to implement subclasses of Object in Rust
//!
//! @author Paolo Bonzini

#![allow(clippy::missing_safety_doc)]

use const_default::ConstDefault;

use std::ffi::c_void;
use std::mem;
use std::mem::MaybeUninit;
use std::ptr::drop_in_place;

use crate::qom::object::ObjectType;

use crate::qom::refs::ObjectCast;

use crate::bindings::type_register;
use crate::bindings::Object;
use crate::bindings::ObjectClass;
use crate::bindings::TypeInfo;

use crate::util::zeroed::Zeroed;

/// Information on which superclass methods are overridden
/// by a Rust-implemented subclass of Object.
pub trait ObjectImpl: ObjectType {
    /// If not `None`, a function that implements the `unparent` member
    /// of the QOM `ObjectClass`.
    const UNPARENT: Option<fn(obj: &Self)> = None;
}

impl ObjectClass {
    /// Initialize an `ObjectClass` from an `ObjectImpl`.
    pub fn class_init<T: ObjectImpl>(&mut self) {
        unsafe extern "C" fn rust_unparent<T: ObjectImpl>(obj: *mut Object) {
            let f = T::UNPARENT.unwrap();
            f((&*obj).unsafe_cast::<T>())
        }
        self.unparent = T::UNPARENT.map(|_| rust_unparent::<T> as _);
    }
}

impl Object {
    pub unsafe extern "C" fn rust_class_init<T: ObjectImpl>(
        klass: *mut c_void,
        _data: *mut c_void,
    ) {
        let oc: &mut ObjectClass = &mut *(klass.cast());
        oc.class_init::<T>();
    }
}

/// Internal information on a Rust-implemented subclass of Object.
/// Only public because it is used by macros.
pub unsafe trait TypeImpl: ObjectType + ObjectImpl {
    type Super: ObjectType;
    type Conf: ConstDefault;
    type State: Default;

    const CLASS_INIT: unsafe extern "C" fn(klass: *mut c_void, data: *mut c_void);

    fn uninit_conf(obj: &mut MaybeUninit<Self>) -> &mut MaybeUninit<Self::Conf>;
    fn uninit_state(obj: &mut MaybeUninit<Self>) -> &mut MaybeUninit<Self::State>;
}

unsafe fn rust_type_register<T: TypeImpl + ObjectImpl>() {
    unsafe extern "C" fn rust_instance_mem_init<T: TypeImpl>(obj: *mut c_void) {
        let obj: &mut std::mem::MaybeUninit<T> = &mut *(obj.cast());

        T::uninit_conf(obj).write(ConstDefault::DEFAULT);
        T::uninit_state(obj).write(Default::default());
    }

    unsafe extern "C" fn rust_instance_finalize<T: TypeImpl>(obj: *mut c_void) {
        let obj: *mut T = obj.cast();
        drop_in_place(obj);
    }

    let ti = TypeInfo {
        name: T::TYPE.as_ptr(),
        parent: T::Super::TYPE.as_ptr(),
        instance_size: mem::size_of::<T>(),
        instance_mem_init: Some(rust_instance_mem_init::<T>),
        instance_finalize: Some(rust_instance_finalize::<T>),
        class_init: Some(T::CLASS_INIT),

        // SAFETY: TypeInfo is defined in C and all fields are okay to be zeroed
        ..Zeroed::zeroed()
    };

    type_register(&ti)
}

#[macro_export]
macro_rules! qom_define_type {
    ($name:expr, $struct:ident, $conf_ty:ty, $state_ty:ty; @extends $super:ty $(,$supers:ty)*) => {
        $crate::with_offsets! {
            #[repr(C)]
            struct $struct {
                // self.base dropped by call to superclass instance_finalize
                base: std::mem::ManuallyDrop<$super>,
                conf: $conf_ty,
                state: $state_ty,
            }
        }

        // Define IsA markers for the struct itself and all the superclasses
        $crate::qom_isa!($struct, $super $(,$supers)*);

        unsafe impl $crate::qom::object::ObjectType for $struct {
            const TYPE: &'static std::ffi::CStr = $name;
        }

        unsafe impl $crate::qom::object_impl::TypeImpl for $struct {
            type Super = $super;
            type Conf = $conf_ty;
            type State = $state_ty;

            const CLASS_INIT: unsafe extern "C" fn(klass: *mut std::ffi::c_void, data: *mut std::ffi::c_void)
                = <$super>::rust_class_init::<Self>;

            fn uninit_conf(obj: &mut std::mem::MaybeUninit::<Self>) -> &mut std::mem::MaybeUninit<$conf_ty> {
                use std::ptr::addr_of_mut;

                // Projecting the incoming reference to a single field is safe,
                // because the return value is also MaybeUnit
                unsafe { &mut *(addr_of_mut!((*obj.as_mut_ptr()).conf).cast()) }
            }

            fn uninit_state(obj: &mut std::mem::MaybeUninit::<Self>) -> &mut std::mem::MaybeUninit<$state_ty> {
                use std::ptr::addr_of_mut;

                // Projecting the incoming reference to a single field is safe,
                // because the return value is also MaybeUnit
                unsafe { &mut *(addr_of_mut!((*obj.as_mut_ptr()).state).cast()) }
            }
        }

        // TODO: call rust_type_register
    };
}

#[macro_export]
macro_rules! conf_type {
    ($type:ty) => {
        <$type as $crate::qom::object_impl::TypeImpl>::Conf
    };
}
