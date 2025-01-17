// TODO: change to use .cast() etc.
#![allow(clippy::ptr_as_ptr)]

/// Traits to map between C structs and native Rust types.
/// Similar to glib-rs but a bit simpler and possibly more
/// idiomatic.
use libc::c_char;
use std::ffi::{c_void, CStr, CString};
use std::fmt;
use std::fmt::Debug;
use std::marker::PhantomData;
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
    ///
    /// ```
    /// # use qemu::CloneToForeign;
    /// let foreign = "Hello, world!".clone_to_foreign();
    /// unsafe {
    ///     String::free_foreign(foreign.into_inner());
    /// }
    /// ```
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
    ///
    /// ```
    /// # use qemu::{CloneToForeign, IntoNative};
    /// let s = "Hello, world!".to_string();
    /// let foreign = s.clone_to_foreign();
    /// let native: String = unsafe {
    ///     foreign.into_native()
    ///     // foreign is not leaked
    /// };
    /// assert_eq!(s, native);
    /// ```
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
    ///
    /// ```
    /// # use qemu::FromForeign;
    /// let p = c"Hello, world!".as_ptr();
    /// let s = unsafe {
    ///     String::cloned_from_foreign(p as *const libc::c_char)
    /// };
    /// assert_eq!(s, "Hello, world!");
    /// ```
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
    ///
    /// ```
    /// # use qemu::{CloneToForeign, FromForeign};
    /// let s = "Hello, world!";
    /// let foreign = s.clone_to_foreign();
    /// unsafe {
    ///     assert_eq!(String::from_foreign(foreign.into_inner()), s);
    /// }
    /// // foreign is not leaked
    /// ```
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
    /// ```
    /// # use qemu::{CloneToForeign, OwnedPointer};
    /// let s = "Hello, world!";
    /// let foreign_str = s.clone_to_foreign();
    /// let foreign_string = OwnedPointer::<String>::from(foreign_str);
    /// # assert_eq!(foreign_string.into_native(), s);
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
    /// ```
    /// # use qemu::{CloneToForeign, OwnedPointer};
    /// let s = "Hello, world!";
    /// let foreign_str = s.clone_to_foreign();
    /// let foreign_string: OwnedPointer<String> = foreign_str.into();
    /// # assert_eq!(foreign_string.into_native(), s);
    pub fn into<U>(self) -> OwnedPointer<U>
    where
        U: CloneToForeign<Foreign = <T as CloneToForeign>::Foreign>,
    {
        OwnedPointer::from(self)
    }

    /// Return the pointer that is stored in the `OwnedPointer`.  The
    /// pointer is valid for as long as the `OwnedPointer` itself.
    ///
    /// ```
    /// # use qemu::CloneToForeign;
    /// let s = "Hello, world!";
    /// let foreign = s.clone_to_foreign();
    /// let p = foreign.as_ptr();
    /// let len = unsafe { libc::strlen(p) };
    /// drop(foreign);
    /// # assert_eq!(len, 13);
    /// ```
    pub fn as_ptr(&self) -> *const <T as CloneToForeign>::Foreign {
        self.ptr
    }

    pub fn as_mut_ptr(&self) -> *mut <T as CloneToForeign>::Foreign {
        self.ptr
    }

    /// Return the pointer that is stored in the `OwnedPointer`,
    /// consuming the `OwnedPointer` but not freeing the pointer.
    ///
    /// ```
    /// # use qemu::CloneToForeign;
    /// let s = "Hello, world!";
    /// let p = s.clone_to_foreign().into_inner();
    /// let len = unsafe { libc::strlen(p) };
    /// // p needs to be freed manually
    /// # assert_eq!(len, 13);
    /// ```
    pub fn into_inner(mut self) -> *mut <T as CloneToForeign>::Foreign {
        let result = mem::replace(&mut self.ptr, ptr::null_mut());
        mem::forget(self);
        result
    }
}

impl<T: FromForeign + ?Sized> OwnedPointer<T> {
    /// Convert a C datum to a native Rust object, taking ownership of
    /// the pointer or Rust object (same as `from_glib_full` in `glib-rs`)
    ///
    /// ```
    /// # use qemu::{CloneToForeign, IntoNative};
    /// let s = "Hello, world!".to_string();
    /// let foreign = s.clone_to_foreign();
    /// let native: String = unsafe {
    ///     foreign.into_native()
    ///     // foreign is not leaked
    /// };
    /// assert_eq!(s, native);
    /// ```
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

/// A pointer whose contents were borrowed from a Rust object, and
/// therefore whose lifetime is limited to the lifetime of the
/// underlying Rust object.  The Rust object was borrowed from a
/// shared reference, and therefore the pointer is not mutable.
pub struct BorrowedPointer<'a, P, T: 'a> {
    ptr: *const P,
    storage: T,
    _marker: PhantomData<&'a P>,
}

impl<'a, P, T: 'a> BorrowedPointer<'a, P, T> {
    /// Return a new `BorrowedPointer` that wraps the pointer `ptr`.
    /// `storage` can contain any other data that `ptr` points to,
    /// and that should be dropped when the `BorrowedPointer` goes out
    /// of scope.
    pub fn new(ptr: *const P, storage: T) -> Self {
        BorrowedPointer {
            ptr,
            storage,
            _marker: PhantomData,
        }
    }

    /// Return the pointer that is stored in the `BorrowedPointer`.  The
    /// pointer is valid for as long as the `BorrowedPointer` itself.
    ///
    /// ```
    /// # use qemu::ForeignBorrow;
    /// let s = "Hello, world!".to_string();
    /// let borrowed = s.borrow_foreign();
    /// let len = unsafe { libc::strlen(borrowed.as_ptr()) };
    /// # assert_eq!(len, 13);
    /// ```
    pub fn as_ptr(&self) -> *const P {
        self.ptr
    }

    fn map<U: 'a, F: FnOnce(T) -> U>(self, f: F) -> BorrowedPointer<'a, P, U> {
        BorrowedPointer {
            ptr: self.ptr,
            storage: f(self.storage),
            _marker: PhantomData,
        }
    }
}

impl<'a, P, T: 'a> Debug for BorrowedPointer<'a, P, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr_name = std::any::type_name::<*mut P>();
        let storage_name = std::any::type_name::<T>();
        let name = format!("BorrowedPointer<{}, {}>", ptr_name, storage_name);
        f.debug_tuple(&name).field(&self.as_ptr()).finish()
    }
}

/// A pointer whose contents were borrowed from a Rust object, and
/// therefore whose lifetime is limited to the lifetime of the
/// underlying Rust object.  The Rust object is borrowed from an
/// exclusive reference, and therefore the pointer is mutable.
pub struct BorrowedMutPointer<'a, P, T: 'a> {
    ptr: *mut P,
    storage: T,
    _marker: PhantomData<&'a P>,
}

impl<'a, P, T: 'a> BorrowedMutPointer<'a, P, T> {
    /// Return a new `BorrowedMutPointer` that wraps the pointer `ptr`.
    /// `storage` can contain any other data that `ptr` points to,
    /// and that should be dropped when the `BorrowedMutPointer` goes out
    /// of scope.
    pub fn new(ptr: *mut P, storage: T) -> Self {
        BorrowedMutPointer {
            ptr,
            storage,
            _marker: PhantomData,
        }
    }

    /// Return the pointer that is stored in the `BorrowedPointer`.  The
    /// returned pointer is constant and is valid for as long as the
    /// `BorrowedPointer` itself.
    pub fn as_ptr(&self) -> *const P {
        self.ptr
    }

    /// Return the pointer that is stored in the `BorrowedPointer`.  The
    /// returned pointer is mutable and is valid for as long as the
    /// `BorrowedPointer` itself.
    pub fn as_mut_ptr(&mut self) -> *mut P {
        self.ptr
    }

    fn map<U: 'a, F: FnOnce(T) -> U>(self, f: F) -> BorrowedMutPointer<'a, P, U> {
        BorrowedMutPointer {
            ptr: self.ptr,
            storage: f(self.storage),
            _marker: PhantomData,
        }
    }
}

impl<'a, P, T: 'a> Debug for BorrowedMutPointer<'a, P, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = std::any::type_name::<*mut P>();
        let name = format!("BorrowedMutPointer<{}>", name);
        f.debug_tuple(&name).field(&self.as_ptr()).finish()
    }
}

/// A type for which a C representation can be borrowed without cloning.
pub trait ForeignBorrow<'a>: CloneToForeign {
    /// The type of any extra data that are needed while the `BorrowedPointer` is alive.
    type Storage: 'a;

    /// Return a wrapper for a C representation of `self`.  The wrapper
    /// allows access via a constant pointer.
    ///
    /// ```
    /// # use qemu::ForeignBorrow;
    /// let s = "Hello, world!".to_string();
    /// let borrowed = s.borrow_foreign();
    /// let len = unsafe { libc::strlen(borrowed.as_ptr()) };
    /// # assert_eq!(len, 13);
    /// ```
    fn borrow_foreign(&'a self) -> BorrowedPointer<'a, Self::Foreign, Self::Storage>;
}

impl<'a, T> ForeignBorrow<'a> for Option<T>
where
    T: ForeignBorrow<'a>,
{
    type Storage = Option<<T as ForeignBorrow<'a>>::Storage>;

    fn borrow_foreign(&'a self) -> BorrowedPointer<'a, Self::Foreign, Self::Storage> {
        match self.as_ref().map(ForeignBorrow::borrow_foreign) {
            None => BorrowedPointer::new(ptr::null(), None),
            Some(bp) => bp.map(Some),
        }
    }
}

impl<'a, T> ForeignBorrow<'a> for Box<T>
where
    T: ForeignBorrow<'a>,
{
    type Storage = <T as ForeignBorrow<'a>>::Storage;

    fn borrow_foreign(&'a self) -> BorrowedPointer<'a, Self::Foreign, Self::Storage> {
        self.as_ref().borrow_foreign()
    }
}

/// A type for which a C representation can be borrowed mutably without cloning.
pub trait ForeignBorrowMut<'a>: CloneToForeign {
    /// The type of any extra data that are needed while the `BorrowedPointer` is alive.
    type Storage: 'a;

    /// Return a wrapper for a C representation of `self`.  The wrapper
    /// allows access via a mutable pointer.
    ///
    /// ```
    /// # use qemu::ForeignBorrowMut;
    /// let mut i = 123i8;
    /// let mut borrowed = i.borrow_foreign_mut();
    /// unsafe {
    ///     assert_eq!(*borrowed.as_ptr(), 123i8);
    ///     *borrowed.as_mut_ptr() = 45i8;
    /// }
    /// assert_eq!(i, 45i8);
    /// ```
    /// is analogous to:
    /// ```
    /// let mut i = 123i8;
    /// let borrowed = &mut i;
    /// assert_eq!(*borrowed, 123i8);
    /// *borrowed = 45i8;
    /// assert_eq!(i, 45i8);
    /// ```
    fn borrow_foreign_mut(&'a mut self) -> BorrowedMutPointer<'a, Self::Foreign, Self::Storage>;
}

impl<'a, T> ForeignBorrowMut<'a> for Option<T>
where
    T: ForeignBorrowMut<'a>,
{
    type Storage = Option<<T as ForeignBorrowMut<'a>>::Storage>;

    fn borrow_foreign_mut(&'a mut self) -> BorrowedMutPointer<'a, T::Foreign, Self::Storage> {
        match self.as_mut().map(ForeignBorrowMut::borrow_foreign_mut) {
            None => BorrowedMutPointer::new(ptr::null_mut(), None),
            Some(bp) => bp.map(Some),
        }
    }
}

impl<'a, T> ForeignBorrowMut<'a> for Box<T>
where
    T: ForeignBorrowMut<'a>,
{
    type Storage = <T as ForeignBorrowMut<'a>>::Storage;

    fn borrow_foreign_mut(&'a mut self) -> BorrowedMutPointer<'a, Self::Foreign, Self::Storage> {
        self.as_mut().borrow_foreign_mut()
    }
}

impl CloneToForeign for str {
    type Foreign = c_char;

    unsafe fn free_foreign(ptr: *mut c_char) {
        libc::free(ptr as *mut c_void);
    }

    fn clone_to_foreign(&self) -> OwnedPointer<Self> {
        // SAFETY: self.as_ptr() is guaranteed to point to self.len() bytes;
        // the destination is freshly allocated
        unsafe {
            let p = libc::malloc(self.len() + 1) as *mut c_char;
            ptr::copy_nonoverlapping(self.as_ptr() as *const c_char, p, self.len());
            *p.add(self.len()) = 0;
            OwnedPointer::new(p)
        }
    }
}

impl CloneToForeign for String {
    type Foreign = c_char;

    unsafe fn free_foreign(ptr: *mut c_char) {
        libc::free(ptr as *mut c_void);
    }

    fn clone_to_foreign(&self) -> OwnedPointer<Self> {
        // SAFETY: self.as_ptr() is guaranteed to point to self.len() bytes;
        // the destination is freshly allocated
        unsafe {
            let p = libc::malloc(self.len() + 1) as *mut c_char;
            ptr::copy_nonoverlapping(self.as_ptr() as *const c_char, p, self.len());
            *p.add(self.len()) = 0;
            OwnedPointer::new(p)
        }
    }
}

impl FromForeign for String {
    unsafe fn cloned_from_foreign(p: *const c_char) -> Self {
        let cstr = CStr::from_ptr(p);
        String::from_utf8_lossy(cstr.to_bytes()).into_owned()
    }
}

impl ForeignBorrow<'_> for String {
    type Storage = CString;

    fn borrow_foreign(&self) -> BorrowedPointer<c_char, CString> {
        let tmp = CString::new(&self[..]).unwrap();
        BorrowedPointer::new(tmp.as_ptr(), tmp)
    }
}

macro_rules! foreign_copy_type {
    ($rust_type:ty, $foreign_type:ty) => {
        impl CloneToForeign for $rust_type {
            type Foreign = $foreign_type;

            unsafe fn free_foreign(ptr: *mut Self::Foreign) {
                libc::free(ptr as *mut c_void);
            }

            fn clone_to_foreign(&self) -> OwnedPointer<Self> {
                // Safety: we are copying into a freshly-allocated block
                unsafe {
                    let p = libc::malloc(mem::size_of::<Self>()) as *mut Self::Foreign;
                    *p = *self as Self::Foreign;
                    OwnedPointer::new(p)
                }
            }
        }

        impl FromForeign for $rust_type {
            unsafe fn cloned_from_foreign(p: *const Self::Foreign) -> Self {
                *p
            }
        }

        impl<'a> ForeignBorrow<'a> for $rust_type {
            type Storage = &'a Self;

            fn borrow_foreign(&self) -> BorrowedPointer<Self::Foreign, &Self> {
                BorrowedPointer::new(self, self)
            }
        }

        impl<'a> ForeignBorrowMut<'a> for $rust_type {
            type Storage = &'a mut Self;

            fn borrow_foreign_mut(&'a mut self) -> BorrowedMutPointer<Self::Foreign, &'a mut Self> {
                BorrowedMutPointer::new(self, self)
            }
        }

        impl CloneToForeign for [$rust_type] {
            type Foreign = $foreign_type;

            unsafe fn free_foreign(ptr: *mut Self::Foreign) {
                libc::free(ptr as *mut c_void);
            }

            fn clone_to_foreign(&self) -> OwnedPointer<Self> {
                // SAFETY: self.as_ptr() is guaranteed to point to the same number of bytes
                // as the freshly allocated destination
                unsafe {
                    let size = mem::size_of::<Self::Foreign>();
                    let p = libc::malloc(self.len() * size) as *mut Self::Foreign;
                    ptr::copy_nonoverlapping(self.as_ptr() as *const Self::Foreign, p, self.len());
                    OwnedPointer::new(p)
                }
            }
        }

        impl<'a> ForeignBorrow<'a> for [$rust_type] {
            type Storage = &'a Self;

            fn borrow_foreign(&self) -> BorrowedPointer<Self::Foreign, &Self> {
                BorrowedPointer::new(self.as_ptr(), self)
            }
        }

        impl<'a> ForeignBorrowMut<'a> for [$rust_type] {
            type Storage = &'a mut Self;

            fn borrow_foreign_mut(&'a mut self) -> BorrowedMutPointer<Self::Foreign, &'a mut Self> {
                BorrowedMutPointer::new(self.as_mut_ptr(), self)
            }
        }
    };
}
foreign_copy_type!(i8, i8);
foreign_copy_type!(u8, u8);
foreign_copy_type!(i16, i16);
foreign_copy_type!(u16, u16);
foreign_copy_type!(i32, i32);
foreign_copy_type!(u32, u32);
foreign_copy_type!(i64, i64);
foreign_copy_type!(u64, u64);
foreign_copy_type!(isize, libc::ptrdiff_t);
foreign_copy_type!(usize, libc::size_t);
foreign_copy_type!(f32, f32);
foreign_copy_type!(f64, f64);

#[cfg(test)]
mod tests {
    #![allow(clippy::shadow_unrelated)]

    use super::*;
    use matches::assert_matches;
    use std::ffi::c_void;

    #[test]
    fn test_foreign_int_convert() {
        let i = 123i8;
        let p = i.clone_to_foreign();
        unsafe {
            assert_eq!(i, *p.as_ptr());
            assert_eq!(i, i8::cloned_from_foreign(p.as_ptr()));
        }
        assert_eq!(i, p.into_native());

        let p = i.clone_to_foreign();
        unsafe {
            assert_eq!(i, i8::from_foreign(p.into_inner()));
        }
    }

    #[test]
    fn test_foreign_int_borrow() {
        let i = 123i8;
        unsafe {
            assert_eq!(i, *i.borrow_foreign().as_ptr());
        }
        assert_eq!(i, 123i8);
    }

    #[test]
    fn test_foreign_int_borrow_mut() {
        let mut i = 123i8;
        let mut borrowed = i.borrow_foreign_mut();
        unsafe {
            assert_eq!(*borrowed.as_ptr(), 123i8);
            *borrowed.as_mut_ptr() = 45i8;
        }
        let borrowed = i.borrow_foreign_mut();
        unsafe {
            assert_eq!(*borrowed.as_ptr(), 45i8);
        }
        assert_eq!(i, 45i8);
    }

    #[test]
    fn test_borrow_foreign_string() {
        let s = "Hello, world!".to_string();
        let borrowed = s.borrow_foreign();
        unsafe {
            let len = libc::strlen(borrowed.as_ptr());
            assert_eq!(len, s.len());
            assert_eq!(
                libc::memcmp(
                    borrowed.as_ptr() as *const c_void,
                    "Hello, world!\0".as_bytes().as_ptr() as *const c_void,
                    len + 1
                ),
                0
            );
        }
    }

    #[test]
    fn test_cloned_from_foreign_string() {
        let s = "Hello, world!".to_string();
        let borrowed = s.borrow_foreign();
        let cloned = unsafe { String::cloned_from_foreign(s.borrow_foreign().as_ptr()) };
        assert_eq!(s, cloned);
        assert_ne!(s.borrow_foreign().as_ptr(), borrowed.as_ptr());
    }

    #[test]
    fn test_from_foreign_string() {
        let s = "Hello, world!".to_string();
        let cloned = s.clone_to_foreign_ptr();
        let copy = unsafe { String::from_foreign(cloned) };
        assert_eq!(s, copy);
    }

    #[test]
    fn test_owned_pointer_default() {
        let s: String = Default::default();
        let foreign: OwnedPointer<String> = Default::default();
        let native = foreign.into_native();
        assert_eq!(s, native);
    }

    #[test]
    fn test_owned_pointer_into() {
        let s = "Hello, world!".to_string();
        let cloned: OwnedPointer<String> = s.clone_to_foreign().into();
        let copy = cloned.into_native();
        assert_eq!(s, copy);
    }

    #[test]
    fn test_owned_pointer_into_native() {
        let s = "Hello, world!".to_string();
        let cloned = s.clone_to_foreign();
        let copy = cloned.into_native();
        assert_eq!(s, copy);
    }

    #[test]
    fn test_ptr_into_native() {
        let s = "Hello, world!".to_string();
        let cloned = s.clone_to_foreign_ptr();
        let copy: String = unsafe { cloned.into_native() };
        assert_eq!(s, copy);

        // This is why type bounds are needed... they aren't for
        // OwnedPointer::into_native
        let cloned = s.clone_to_foreign_ptr();
        let copy: c_char = unsafe { cloned.into_native() };
        assert_eq!(s.as_bytes()[0], copy as u8);
    }

    #[test]
    fn test_clone_to_foreign_str() {
        let s = "Hello, world!";
        let p = c"Hello, world!".as_ptr();
        let cloned = s.clone_to_foreign();
        unsafe {
            let len = libc::strlen(cloned.as_ptr());
            assert_eq!(len, s.len());
            assert_eq!(
                libc::memcmp(
                    cloned.as_ptr() as *const c_void,
                    p as *const c_void,
                    len + 1
                ),
                0
            );
        }
    }

    #[test]
    fn test_clone_to_foreign_bytes() {
        let s = b"Hello, world!\0";
        let cloned = s.clone_to_foreign();
        unsafe {
            let len = libc::strlen(cloned.as_ptr() as *const c_char);
            assert_eq!(len, s.len() - 1);
            assert_eq!(
                libc::memcmp(
                    cloned.as_ptr() as *const c_void,
                    s.as_ptr() as *const c_void,
                    len + 1
                ),
                0
            );
        }
    }

    #[test]
    fn test_borrow_foreign_bytes() {
        let s = b"Hello, world!\0";
        let borrowed = s.borrow_foreign();
        unsafe {
            let len = libc::strlen(borrowed.as_ptr() as *const c_char);
            assert_eq!(len, s.len() - 1);
            assert_eq!(
                libc::memcmp(
                    borrowed.as_ptr() as *const c_void,
                    s.as_ptr() as *const c_void,
                    len + 1
                ),
                0
            );
        }
    }

    #[test]
    fn test_clone_to_foreign_string() {
        let s = "Hello, world!".to_string();
        let borrowed = s.borrow_foreign();
        let cloned = s.clone_to_foreign();
        assert_ne!(borrowed.as_ptr(), cloned.as_ptr());
        unsafe {
            let len = libc::strlen(cloned.as_ptr());
            assert_eq!(len, s.len());
            assert_eq!(
                libc::memcmp(
                    cloned.as_ptr() as *const c_void,
                    borrowed.as_ptr() as *const c_void,
                    len + 1
                ),
                0
            );
        }
    }

    #[test]
    fn test_option() {
        // An Option can be used to produce or convert NULL pointers
        let s = Some("Hello, world!".to_string());
        unsafe {
            assert_eq!(
                Option::<String>::cloned_from_foreign(s.borrow_foreign().as_ptr()),
                s
            );
        }

        let s: Option<String> = None;
        assert_eq!(s.borrow_foreign().as_ptr(), ptr::null());
        unsafe {
            assert_matches!(Option::<String>::cloned_from_foreign(ptr::null()), None);
            assert_matches!(Option::<String>::from_foreign(ptr::null_mut()), None);
        }
    }

    #[test]
    fn test_box() {
        // A box can be produced if the inner type has the capability.
        // Contents of a Box can be borrowed.
        let s = Box::new("Hello, world!".to_string());
        let borrowed = s.borrow_foreign();
        let cloned = unsafe { Box::<String>::cloned_from_foreign(borrowed.as_ptr()) };
        assert_eq!(s, cloned);

        let s = Some(Box::new("Hello, world!".to_string()));
        let borrowed = s.borrow_foreign();
        let cloned = unsafe { Option::<Box<String>>::cloned_from_foreign(borrowed.as_ptr()) };
        assert_eq!(s, cloned);
    }
}
