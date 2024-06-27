//! Casting and reference counting traits for QOM objects
//!
//! @author Paolo Bonzini

use crate::bindings::object_dynamic_cast;
use crate::bindings::Object;
use crate::bindings::{object_ref, object_unref};

use crate::qom::object::ObjectMethods;
use crate::qom::object::ObjectType;

use std::borrow::Borrow;
use std::fmt::{self, Debug};
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ptr::NonNull;

/// Marker trait: `Self` can be statically upcasted to `P` (i.e. `P` is a direct
/// or indirect parent of `Self`).
///
/// # Safety
///
/// The struct `Self` must begin, directly or indirectly, with a field of type
/// `P`.  This ensures that invalid casts, which rely on `IsA<>` for static
/// checking, are rejected at compile time.
pub unsafe trait IsA<P: ObjectType>: ObjectType {}

// SAFETY: it is always safe to cast to your own type
unsafe impl<T: ObjectType> IsA<T> for T {}

#[macro_export]
macro_rules! qom_isa {
    ($struct:ty $(,$parent:ty)* ) => {
        $(
            impl AsRef<$parent> for $struct {
                fn as_ref(&self) -> &$parent {
                    use $crate::ObjectCast;
                    self.upcast::<$parent>()
                }
            }

            // SAFETY: it is the caller responsibility to have $parent as the
            // first field
            unsafe impl $crate::qom::refs::IsA<$parent> for $struct {}
        )*
    };
}

/// Trait for a reference to a QOM object.  Allows conversion to/from
/// C objects in generic code.
pub trait ObjectCast: Copy + Deref
where
    Self::Target: ObjectType,
{
    /// Convert this (possibly smart) reference to a basic Rust reference.
    fn as_ref(&self) -> &Self::Target {
        self.deref()
    }

    /// Convert to a const Rust pointer, to be used for example for FFI.
    fn as_ptr(&self) -> *const Self::Target {
        self.as_ref()
    }

    /// Convert to a mutable Rust pointer, to be used for example for FFI.
    /// Used to implement interior mutability for objects.
    ///
    /// # Safety
    ///
    /// This method is unsafe because it overrides const-ness of `&self`.
    /// Bindings to C APIs will use it a lot, but otherwise it should not
    /// be necessary.
    unsafe fn as_mut_ptr(&self) -> *mut Self::Target {
        #[allow(clippy::as_ptr_cast_mut)]
        {
            self.as_ptr().cast_mut()
        }
    }

    /// Perform a cast to a superclass
    fn upcast<'a, U: ObjectType>(self) -> &'a U
    where
        Self::Target: IsA<U>,
        Self: 'a,
    {
        // SAFETY: soundness is declared via IsA<U>, which is an unsafe trait
        unsafe { self.unsafe_cast::<U>() }
    }

    /// Perform a cast to a subclass.  Checks at compile time that the
    /// cast can succeed, but the final verification will happen at
    /// runtime only.
    fn downcast<'a, U: IsA<Self::Target>>(self) -> Option<&'a U>
    where
        Self: 'a,
    {
        self.dynamic_cast::<U>()
    }

    /// Perform a cast between QOM types.  The check that U is indeed
    /// the dynamic type of `self` happens at runtime.
    fn dynamic_cast<'a, U: ObjectType>(self) -> Option<&'a U>
    where
        Self: 'a,
    {
        unsafe {
            // SAFETY: upcasting to Object is always valid, and the
            // return type is either NULL or the argument itself
            let result: *const U =
                object_dynamic_cast(self.unsafe_cast::<Object>().as_mut_ptr(), U::TYPE.as_ptr())
                    .cast();

            result.as_ref()
        }
    }

    /// Unconditional cast to an arbitrary QOM type.
    ///
    /// # Safety
    ///
    /// What safety? You need to know yourself that the cast is correct; only use
    /// when performance is paramount.  It is still better than a raw pointer
    /// `cast()`, which does not even check that you remain in the realm of
    /// QOM `ObjectType`s.
    ///
    /// `unsafe_cast::<Object>()` can also be used, and is always safe, if all
    /// you have is an `ObjectType` (as opposed to an `IsA<Object>`).
    unsafe fn unsafe_cast<'a, U: ObjectType>(self) -> &'a U
    where
        Self: 'a,
    {
        &*(self.as_ptr().cast::<U>())
    }
}

impl<T: ObjectType> ObjectCast for &T {}

/// An owned reference to a QOM object.
///
/// Like [`std::sync::Arc`], references are added with [`Clone::clone`] and removed
/// by dropping the `Arc`.
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Arc<T: ObjectType>(NonNull<T>);

// QOM knows how to handle reference counting across threads, but sending
// the Arc to another thread requires the implementation itself to be
// thread-safe (aka Sync).  But I'm not entirely sure that this is enough
// (see for example ARef in rust/kernel/types.rs, which is very similar
// to this type).
//
//unsafe impl<T: Sync + ObjectType> Send for Arc<T> {}
//unsafe impl<T: ObjectType> Sync for Arc<T> {}

impl<T: ObjectType> Arc<T> {
    /// Obtain a reference from a raw C pointer
    ///
    /// # Safety
    ///
    /// Typically this function will only be used by low level bindings
    /// to C APIs.
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        // SAFETY NOTE: while NonNull requires a mutable pointer,
        // only Deref is implemented so the pointer passed to from_raw
        // remains const
        Arc(NonNull::new_unchecked(ptr.cast_mut()))
    }

    /// Increase the reference count of a QOM object and return
    ///
    /// # Safety
    ///
    /// Unsafe because the object could be embedded in another.  To
    /// obtain an `Arc` safely, use `ObjectType::new()`.
    pub unsafe fn from(obj: &T) -> Self {
        object_ref(obj.unsafe_cast::<Object>().as_mut_ptr());

        // SAFETY NOTE: while NonNull requires a mutable pointer,
        // only Deref is implemented so the pointer passed to from_raw
        // remains const
        Arc(NonNull::new_unchecked(obj.as_mut_ptr()))
    }

    /// Perform a cast to a superclass
    pub fn upcast<U: ObjectType>(src: Arc<T>) -> Arc<U>
    where
        T: IsA<U>,
    {
        // SAFETY: soundness is declared via IsA<U>, which is an unsafe trait
        unsafe { Arc::unsafe_cast::<U>(src) }
    }

    /// Perform a cast to a subclass.  Checks at compile time that the
    /// cast can succeed, but the final verification will happen at
    /// runtime only.
    pub fn downcast<U: IsA<T>>(src: Arc<T>) -> Result<Arc<U>, Arc<T>> {
        Arc::dynamic_cast::<U>(src)
    }

    /// Perform a cast between QOM types.  The check that U is indeed
    /// the dynamic type of `self` happens at runtime.
    pub fn dynamic_cast<U: ObjectType>(src: Arc<T>) -> Result<Arc<U>, Arc<T>> {
        // override automatic drop to skip the unref/ref
        let src = ManuallyDrop::new(src);
        match src.dynamic_cast::<U>() {
            // get the ownership back from the ManuallyDrop<>
            None => Err(ManuallyDrop::into_inner(src)),

            // SAFETY: the ref is moved (thanks to ManuallyDrop) from
            // self to casted_ref
            Some(casted_ref) => Ok(unsafe { Arc::<U>::from_raw(casted_ref) }),
        }
    }

    /// Unconditional cast to an arbitrary QOM type.
    ///
    /// # Safety
    ///
    /// What safety? You need to know yourself that the cast is correct.  Only use
    /// when performance is paramount
    pub unsafe fn unsafe_cast<U: ObjectType>(src: Arc<T>) -> Arc<U> {
        // override automatic drop to skip the unref/ref
        let src = ManuallyDrop::new(src);
        let casted_ref = src.unsafe_cast::<U>();
        Arc::<U>::from_raw(casted_ref)
    }
}

impl<T: ObjectType> AsRef<T> for Arc<T> {
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl<T: ObjectType> Borrow<T> for Arc<T> {
    fn borrow(&self) -> &T {
        self.deref()
    }
}

impl<T: ObjectType> Clone for Arc<T> {
    fn clone(&self) -> Self {
        // SAFETY: creation method is unsafe, and whoever calls it
        // has responsibility that the pointer is valid
        unsafe { Arc::from(self.deref()) }
    }
}

impl<T: ObjectType> Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: creation method is unsafe, and whoever calls it
        // has responsibility that the pointer has a static lifetime.
        // Once that is guaranteed, reference counting ensures that
        // the object remains alive.
        unsafe { &*self.0.as_ptr() }
    }
}

impl<T: ObjectType> Drop for Arc<T> {
    fn drop(&mut self) {
        // SAFETY: creation method is unsafe, and whoever calls it
        // has responsibility that the pointer is valid
        unsafe {
            object_unref(self.unsafe_cast::<Object>().as_mut_ptr());
        }
    }
}

impl<T: IsA<Object>> Debug for Arc<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.deref().debug_fmt(f)
    }
}
