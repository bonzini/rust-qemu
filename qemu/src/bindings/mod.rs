use libc::c_char;
use std::ffi::c_void;

#[repr(C)]
pub struct Object {
    pub klass: *mut c_void,
    pub free: extern "C" fn(c: *mut c_void),
    pub properties: *mut c_void,
    pub r#ref: u32,
    pub parent: *mut Object,
}

#[repr(C)]
pub struct ObjectClass {
    pub unparent: Option<unsafe extern "C" fn(*mut Object)>,
}

#[repr(C)]
pub struct DeviceState {
    pub base: Object,
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct PropertyInfo {
    pub name: *const c_char,
    pub description: *const c_char,
    // ...
}
#[repr(C)]
pub struct Property {
    pub name: *const c_char,
    pub offset: usize,
    pub default: u64,
    pub info: *const PropertyInfo,
}

pub struct DeviceClass {
    pub oc: ObjectClass,

    pub realize: Option<unsafe extern "C" fn(*mut DeviceState, *mut *mut Error)>,
    pub unrealize: Option<unsafe extern "C" fn(*mut DeviceState)>,
    pub cold_reset: Option<unsafe extern "C" fn(*mut DeviceState)>,
    pub properties: *const Property,
}

#[repr(C)]
pub struct TypeInfo {
    pub name: *const c_char,
    pub parent: *const c_char,
    pub instance_mem_init: Option<unsafe extern "C" fn(*mut c_void)>,
    pub instance_init: Option<unsafe extern "C" fn(*mut c_void)>,
    pub instance_finalize: Option<unsafe extern "C" fn(*mut c_void)>,
    pub class_init: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    pub instance_size: usize,
}

#[repr(C)]
pub struct Error {
    _unused: c_char,
}

extern "C" {
    pub fn error_setg_internal(
        errp: *mut *mut Error,
        src: *mut c_char,
        line: u32,
        func: *mut c_char,
        fmt: *const c_char,
        ...
    );
    pub fn error_get_pretty(errp: *const Error) -> *mut c_char;
    pub fn error_free(errp: *mut Error);

    pub fn object_dynamic_cast(obj: *mut Object, typ: *const c_char) -> *mut c_void;
    pub fn object_get_typename(obj: *const Object) -> *const c_char;
    pub fn object_ref(obj: *mut Object);
    pub fn object_new(typ: *const c_char) -> *const Object;
    pub fn object_unref(obj: *mut Object);
    pub fn object_unparent(obj: *mut Object);

    pub fn device_cold_reset(obj: *mut DeviceState);
    pub fn device_realize(obj: *mut DeviceState, err: *mut *mut Error) -> bool;
    pub fn type_register(obj: *const TypeInfo);

    pub static qdev_prop_bool: PropertyInfo;
}
