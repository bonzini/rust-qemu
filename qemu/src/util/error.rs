//! Error class for QEMU Rust code
//!
//! @author Paolo Bonzini

use crate::bindings;
use crate::bindings::error_free;
use crate::bindings::error_get_pretty;
use crate::bindings::error_setg_internal;

use std::ffi::CStr;
use std::fmt::{self, Display};
use std::ptr;

use crate::util::foreign::{CloneToForeign, FromForeign, OwnedPointer};

#[derive(Debug, Default)]
pub struct Error {
    msg: Option<String>,
    /// Appends the print string of the error to the msg if not None
    cause: Option<Box<dyn std::error::Error>>,
    location: Option<(String, u32)>,
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.as_deref()
    }

    #[allow(deprecated)]
    fn description(&self) -> &str {
        self.msg
            .as_deref()
            .or_else(|| self.cause.as_deref().map(std::error::Error::description))
            .unwrap_or("error")
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut prefix = "";
        if let Some((ref file, line)) = self.location {
            write!(f, "{}:{}", file, line)?;
            prefix = ": ";
        }
        if let Some(ref msg) = self.msg {
            write!(f, "{}{}", prefix, msg)?;
            prefix = ": ";
        }
        if let Some(ref cause) = self.cause {
            write!(f, "{}{}", prefix, cause)?;
        } else if prefix.is_empty() {
            f.write_str("unknown error")?;
        }
        Ok(())
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Error {
            msg: Some(String::from(msg)),
            cause: None,
            location: None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error {
            msg: None,
            cause: Some(Box::new(error)),
            location: None,
        }
    }
}

impl Error {
    /// Create a new error, prepending `msg` to the
    /// description of `cause`
    pub fn with_error<E: std::error::Error + 'static>(msg: &str, cause: E) -> Self {
        Error {
            msg: Some(String::from(msg)),
            cause: Some(Box::new(cause)),
            location: None,
        }
    }

    /// Create a new error, prepending `file:line: msg` to the
    /// description of `cause`
    pub fn with_error_file_line<E: std::error::Error + 'static>(
        msg: &str,
        cause: E,
        file: &str,
        line: u32,
    ) -> Self {
        Error {
            msg: Some(String::from(msg)),
            cause: Some(Box::new(cause)),
            location: Some((String::from(file), line)),
        }
    }

    /// Create a new error with format `file:line: msg`
    pub fn with_file_line(msg: &str, file: &str, line: u32) -> Self {
        Error {
            msg: Some(String::from(msg)),
            cause: None,
            location: Some((String::from(file), line)),
        }
    }

    /// Consume a result, returning false if it is an error and
    /// true if it is successful.  The error is propagated into
    /// `errp` like the C API `error_propagate` would do.
    ///
    /// # Safety
    ///
    /// `errp` must be valid; typically it is received from C code
    pub unsafe fn bool_or_propagate<T>(
        result: Result<(), Self>,
        errp: *mut *mut bindings::Error,
    ) -> bool {
        Self::ok_or_propagate(result, errp).is_some()
    }

    /// Consume a result, returning a `NULL` pointer if it is an
    /// error and the contents if it is successful.  The error is
    /// propagated into `errp` like the C API `error_propagate`
    /// would do
    ///
    /// # Safety
    ///
    /// `errp` must be valid; typically it is received from C code
    pub unsafe fn ptr_or_propagate<T: CloneToForeign>(
        result: Result<T, Self>,
        errp: *mut *mut bindings::Error,
    ) -> *mut T::Foreign {
        Self::ok_or_propagate(result, errp).map_or(ptr::null_mut(), |ref x| {
            CloneToForeign::clone_to_foreign_ptr(x)
        })
    }

    /// Consume a result and return `self.ok()`, but also propagate a
    /// possible error into `errp`, like the C API `error_propagate`
    /// would do.
    ///
    /// # Safety
    ///
    /// `errp` must be valid; typically it is received from C code
    pub unsafe fn ok_or_propagate<T>(
        result: Result<T, Self>,
        errp: *mut *mut bindings::Error,
    ) -> Option<T> {
        match result {
            Ok(ok) => Some(ok),
            Err(err) => {
                err.propagate(errp);
                None
            }
        }
    }

    /// Equivalent of the C function `error_propagate`.  Fill `*errp`
    /// with the information container in `self` if `errp` is not NULL;
    /// then consume it.
    ///
    /// # Safety
    ///
    /// `errp` must be valid; typically it is received from C code
    pub unsafe fn propagate(self, errp: *mut *mut bindings::Error) {
        if errp.is_null() {
            return;
        }
        errp.write(self.clone_to_foreign_ptr());
    }

    /// Convert a C `Error*` into a Rust `Result`, using `Ok(Default::default())`
    /// if `c_error` is NULL.
    ///
    /// # Safety
    ///
    /// `c_error` must be valid; typically it has been filled by a C
    /// function.
    pub unsafe fn err_or_default<T: Default>(c_error: *mut bindings::Error) -> Result<T, Self> {
        Self::err_or_else(c_error, Default::default)
    }

    /// Convert a C `Error*` into a Rust `Result`, calling `f()` to
    /// obtain an `Ok` value if `c_error` is NULL.
    ///
    /// # Safety
    ///
    /// `c_error` must be valid; typically it has been filled by a C
    /// function.
    pub unsafe fn err_or_else<T, F: FnOnce() -> T>(
        c_error: *mut bindings::Error,
        f: F,
    ) -> Result<T, Self> {
        match Option::<Self>::from_foreign(c_error) {
            None => Ok(f()),
            Some(err) => Err(err),
        }
    }
}

impl CloneToForeign for Error {
    type Foreign = bindings::Error;

    fn clone_to_foreign(&self) -> OwnedPointer<Self> {
        let mut x: *mut bindings::Error = ptr::null_mut();
        unsafe {
            error_setg_internal(
                &mut x,
                ptr::null_mut(), // FIXME
                0,
                ptr::null_mut(), // FIXME
                c"%s".as_ptr(),
                format!("{}", self),
            );
            OwnedPointer::new(x)
        }
    }

    unsafe fn free_foreign(p: *mut bindings::Error) {
        unsafe {
            error_free(p);
        }
    }
}

impl FromForeign for Error {
    unsafe fn cloned_from_foreign(c_error: *const bindings::Error) -> Self {
        let c_str = unsafe { CStr::from_ptr(error_get_pretty(c_error)) };
        Error {
            msg: Some(c_str.to_string_lossy().into_owned()),
            cause: None,
            location: None,
        }
    }
}
