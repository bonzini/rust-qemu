// TODO: change to use .cast() etc.
#![allow(clippy::ptr_as_ptr)]

/// Traits to map between C structs and native Rust types.
/// Similar to glib-rs but a bit simpler and possibly more
/// idiomatic.
use std::fmt;
use std::fmt::Debug;
use std::mem;
use std::ptr;

/// A type for which there is a canonical representation as a C datum.
pub trait CloneToForeign {
    /// The representation of `Self` as a C datum.  Typically a
    /// `struct`, though there are exceptions for example `c_char`
    /// for strings, since C strings are of `char *` type).
    type Foreign;

    /// Free the C datum pointed to by `p`.
    ///
    /// # Safety
    ///
    /// `p` must be `NULL` or point to valid data.
    unsafe fn free_foreign(p: *mut Self::Foreign);

    /// Convert a native Rust object to a foreign C struct, copying
    /// everything pointed to by `self` (same as `to_glib_full` in `glib-rs`)
    fn clone_to_foreign(&self) -> OwnedPointer<Self>;

    /// Convert a native Rust object to a foreign C pointer, copying
    /// everything pointed to by `self`.  The returned pointer must
    /// be freed with the `free_foreign` associated function.
    fn clone_to_foreign_ptr(&self) -> *mut Self::Foreign {
        self.clone_to_foreign().into_inner()
    }
}

impl<T> CloneToForeign for Option<T>
where
    T: CloneToForeign,
{
    type Foreign = <T as CloneToForeign>::Foreign;

    unsafe fn free_foreign(x: *mut Self::Foreign) {
        T::free_foreign(x)
    }

    fn clone_to_foreign(&self) -> OwnedPointer<Self> {
        // Same as the underlying implementation, but also convert `None`
        // to a `NULL` pointer.
        self.as_ref()
            .map(CloneToForeign::clone_to_foreign)
            .map(OwnedPointer::into)
            .unwrap_or_default()
    }
}

impl<T> FromForeign for Option<T>
where
    T: FromForeign,
{
    unsafe fn cloned_from_foreign(p: *const Self::Foreign) -> Self {
        // Same as the underlying implementation, but also accept a `NULL` pointer.
        if p.is_null() {
            None
        } else {
            Some(T::cloned_from_foreign(p))
        }
    }
}

impl<T> CloneToForeign for Box<T>
where
    T: CloneToForeign,
{
    type Foreign = <T as CloneToForeign>::Foreign;

    unsafe fn free_foreign(x: *mut Self::Foreign) {
        T::free_foreign(x)
    }

    fn clone_to_foreign(&self) -> OwnedPointer<Self> {
        self.as_ref().clone_to_foreign().into()
    }
}

impl<T> FromForeign for Box<T>
where
    T: FromForeign,
{
    unsafe fn cloned_from_foreign(p: *const Self::Foreign) -> Self {
        Box::new(T::cloned_from_foreign(p))
    }
}

/// Convert a C datum into a native Rust object, taking ownership of
/// the C datum.  You should not need to implement this trait
/// as long as Rust types implement `FromForeign`.
pub trait IntoNative<T> {
    /// Convert a C datum to a native Rust object, taking ownership of
    /// the pointer or Rust object (same as `from_glib_full` in `glib-rs`)
    ///
    /// # Safety
    ///
    /// `p` must point to valid data, or can be `NULL` if Self is an
    /// `Option` type.  It becomes invalid after the function returns.
    unsafe fn into_native(self) -> T;
}

impl<T, U> IntoNative<U> for *mut T
where
    U: FromForeign<Foreign = T>,
{
    unsafe fn into_native(self) -> U {
        U::from_foreign(self)
    }
}

/// A type which can be constructed from a canonical representation as a
/// C datum.
pub trait FromForeign: CloneToForeign + Sized {
    /// Convert a C datum to a native Rust object, copying everything
    /// pointed to by `p` (same as `from_glib_none` in `glib-rs`)
    ///
    /// # Safety
    ///
    /// `p` must point to valid data, or can be `NULL` is `Self` is an
    /// `Option` type.
    unsafe fn cloned_from_foreign(p: *const Self::Foreign) -> Self;

    /// Convert a C datum to a native Rust object, taking ownership of
    /// the pointer or Rust object (same as `from_glib_full` in `glib-rs`)
    ///
    /// The default implementation calls `cloned_from_foreign` and frees `p`.
    ///
    /// # Safety
    ///
    /// `p` must point to valid data, or can be `NULL` is `Self` is an
    /// `Option` type.  `p` becomes invalid after the function returns.
    unsafe fn from_foreign(p: *mut Self::Foreign) -> Self {
        let result = Self::cloned_from_foreign(p);
        Self::free_foreign(p);
        result
    }
}

pub struct OwnedPointer<T: CloneToForeign + ?Sized> {
    ptr: *mut <T as CloneToForeign>::Foreign,
}

impl<T: CloneToForeign + ?Sized> OwnedPointer<T> {
    /// Return a new `OwnedPointer` that wraps the pointer `ptr`.
    ///
    /// # Safety
    ///
    /// The pointer must be valid and live until the returned `OwnedPointer`
    /// is dropped.
    pub unsafe fn new(ptr: *mut <T as CloneToForeign>::Foreign) -> Self {
        OwnedPointer { ptr }
    }

    /// Safely create an `OwnedPointer` from one that has the same
    /// freeing function.
    pub fn from<U>(x: OwnedPointer<U>) -> Self
    where
        U: CloneToForeign<Foreign = <T as CloneToForeign>::Foreign> + ?Sized,
    {
        unsafe {
            // SAFETY: the pointer type and free function are the same,
            // only the type changes
            OwnedPointer::new(x.into_inner())
        }
    }

    /// Safely convert an `OwnedPointer` into one that has the same
    /// freeing function.
    pub fn into<U>(self) -> OwnedPointer<U>
    where
        U: CloneToForeign<Foreign = <T as CloneToForeign>::Foreign>,
    {
        OwnedPointer::from(self)
    }

    /// Return the pointer that is stored in the `OwnedPointer`.  The
    /// pointer is valid for as long as the `OwnedPointer` itself.
    pub fn as_ptr(&self) -> *const <T as CloneToForeign>::Foreign {
        self.ptr
    }

    pub fn as_mut_ptr(&self) -> *mut <T as CloneToForeign>::Foreign {
        self.ptr
    }

    /// Return the pointer that is stored in the `OwnedPointer`,
    /// consuming the `OwnedPointer` but not freeing the pointer.
    pub fn into_inner(mut self) -> *mut <T as CloneToForeign>::Foreign {
        let result = mem::replace(&mut self.ptr, ptr::null_mut());
        mem::forget(self);
        result
    }
}

impl<T: FromForeign + ?Sized> OwnedPointer<T> {
    /// Convert a C datum to a native Rust object, taking ownership of
    /// the pointer or Rust object (same as `from_glib_full` in `glib-rs`)
    pub fn into_native(self) -> T {
        // SAFETY: the pointer was passed to the unsafe constructor OwnedPointer::new
        unsafe { T::from_foreign(self.into_inner()) }
    }
}

impl<T: CloneToForeign + ?Sized> Debug for OwnedPointer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = std::any::type_name::<T>();
        let name = format!("OwnedPointer<{}>", name);
        f.debug_tuple(&name).field(&self.as_ptr()).finish()
    }
}

impl<T: CloneToForeign + Default + ?Sized> Default for OwnedPointer<T> {
    fn default() -> Self {
        <T as Default>::default().clone_to_foreign()
    }
}

impl<T: CloneToForeign + ?Sized> Drop for OwnedPointer<T> {
    fn drop(&mut self) {
        let p = mem::replace(&mut self.ptr, ptr::null_mut());
        // SAFETY: the pointer was passed to the unsafe constructor OwnedPointer::new
        unsafe { T::free_foreign(p) }
    }
}
