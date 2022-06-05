use super::prelude::*;

/// Rust wrapper for `struct dm_block_validator`.
///
/// # Invariant
///
/// The pointer `DmBlockValidator::inner` is non-null and valid.
pub struct DmBlockValidator {
    inner: *mut bindings::dm_block_validator,
}

impl DmBlockValidator {
    /// Create a new block validator
    pub fn new<T: DmBlockValidatorCallbacks>(name: &'static CStr) -> Result<Self> {
        let mut validator = bindings::dm_block_validator::default();
        validator.name = name.as_char_ptr();

        let vtable = unsafe { DmBlockValidatorVTable::<T>::build() };
        validator.prepare_for_write = vtable.prepare_for_write;
        validator.check = vtable.check;

        Ok(Self {
            inner: Box::into_raw(Box::try_new(validator)?),
        })
    }

    /// Get the raw reference of `struct dm_block_validator`.
    ///
    /// # Safety
    ///
    /// Users should not make modification or free the pointer.
    pub unsafe fn raw(&self) -> *mut bindings::dm_block_validator {
        self.inner
    }
}

impl Drop for DmBlockValidator {
    fn drop(&mut self) {
        // SAFETY: Safe. `DmBlockValidator::inner` is a raw pointer made by `DmBlockValidator:new()`.
        let _ = unsafe { Box::from_raw(self.inner) };
    }
}

/// Represents which fields of validator callbacks should be populated
pub struct DmBlockValidatorToUse {
    /// use prepare_for_write
    pub prepare_for_write: bool,
    /// use check
    pub check: bool,
}

/// Validate functions trait helps to build a block validator
#[allow(unused_variables)]
pub trait DmBlockValidatorCallbacks {
    /// The methods to use to populate validate callbacks
    const TO_USE: DmBlockValidatorToUse;

    /// prepare_to_write function
    fn prepare_for_write(block: *mut bindings::dm_block, size: usize) -> Result {
        unimplemented!()
    }

    /// check function
    fn check(block: *mut bindings::dm_block, size: usize) -> Result<i32> {
        unimplemented!()
    }
}

/// Validator callback helper
pub(crate) struct DmBlockValidatorCallbackTable {
    pub prepare_for_write: ::core::option::Option<
        unsafe extern "C" fn(
            v: *mut bindings::dm_block_validator,
            b: *mut bindings::dm_block,
            block_size: usize,
        ),
    >,
    pub check: ::core::option::Option<
        unsafe extern "C" fn(
            v: *mut bindings::dm_block_validator,
            b: *mut bindings::dm_block,
            block_size: usize,
        ) -> c_types::c_int,
    >,
}

struct DmBlockValidatorVTable<T>(marker::PhantomData<T>);

impl<T: DmBlockValidatorCallbacks> DmBlockValidatorVTable<T> {
    unsafe extern "C" fn prepare_for_write(
        _v: *mut bindings::dm_block_validator,
        b: *mut bindings::dm_block,
        block_size: usize,
    ) {
        match T::prepare_for_write(b, block_size) {
            Ok(()) => {}
            Err(e) => pr_warn!("error @ prepare_write: {:?}", e),
        }
    }

    unsafe extern "C" fn check(
        _v: *mut bindings::dm_block_validator,
        b: *mut bindings::dm_block,
        block_size: usize,
    ) -> c_types::c_int {
        match T::check(b, block_size) {
            Ok(val) => val,
            Err(e) => e.to_kernel_errno(),
        }
    }

    const VTABLE: DmBlockValidatorCallbackTable = DmBlockValidatorCallbackTable {
        prepare_for_write: match T::TO_USE.prepare_for_write {
            true => Some(Self::prepare_for_write),
            false => None,
        },
        check: match T::TO_USE.check {
            true => Some(Self::check),
            false => None,
        },
    };

    /// Build the vtable
    pub(crate) const unsafe fn build() -> DmBlockValidatorCallbackTable {
        Self::VTABLE
    }
}
