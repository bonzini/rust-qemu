#![allow(clippy::undocumented_unsafe_blocks)]

use std::mem::MaybeUninit;

/// Trait providing an easy way to obtain an all-zero
/// value for a struct
///
/// # Safety
///
/// Only add this to a type if `MaybeUninit::zeroed().assume_init()`
/// is valid for that type.
pub unsafe trait Zeroed: Sized {
    fn zeroed() -> Self {
        // SAFETY: If this weren't safe, just do not add the
        // trait to a type.
        unsafe { MaybeUninit::zeroed().assume_init() }
    }
}

// Put here all the impls that you need for the bindgen-provided types.
unsafe impl Zeroed for crate::bindings::TypeInfo {}
