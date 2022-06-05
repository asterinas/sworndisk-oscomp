// SPDX-License-Identitifer: GPL-2.0

//! Utility types & functions

use super::prelude::*;

use core::{
    any::{Any, TypeId},
    fmt,
    fmt::Debug,
    pin::Pin,
};

use kernel::{
    error::Result,
    sync::{Mutex, Ref, UniqueRef},
};

#[repr(C)]
/// A vanilla private field wrapper for struct raw pointers
pub struct PrivateField<T: Any + Debug + 'static> {
    lock: Pin<Box<Mutex<()>>>,
    private: T,
    type_id: TypeId,
}

impl<T: Any + Debug + 'static> Debug for PrivateField<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrivateField")
            .field("lock", &(&self.lock as *const _ as usize))
            .field("private", &self.private)
            .field("type_id", &self.type_id)
            .finish()
    }
}

impl<T: Any + Debug + 'static> PrivateField<T> {
    /// Create a new private field wrapper which holds a RwLock
    /// and the private field
    pub fn new(private: T) -> Result<Box<Self>> {
        let mut lock = Pin::from(Box::try_new(
            // SAFETY: rwsemaphore_init! is called below.
            unsafe { Mutex::new(()) },
        )?);

        let mutex = lock.as_mut();
        kernel::mutex_init!(mutex, "PrivateField::lock");

        let field = Box::try_new(Self {
            lock,
            private: private,
            type_id: TypeId::of::<T>(),
        })?;

        Ok(field)
    }

    /// Create a new private field wrapper, and return an Arc (Ref)
    pub fn new_ref(private: T) -> Result<Ref<Self>> {
        let mut lock = Pin::from(Box::try_new(
            // SAFETY: rwsemaphore_init! is called below.
            unsafe { Mutex::new(()) },
        )?);

        kernel::mutex_init!(lock.as_mut(), "PrivateField::lock");

        let field = Pin::from(UniqueRef::try_new(Self {
            lock,
            private: private,
            type_id: TypeId::of::<T>(),
        })?);

        Ok(field.into())
    }

    /// Safely access the private field with a callback.
    pub fn access_private<F, A>(&self, callback: F) -> A
    where
        F: FnOnce(&T) -> A,
    {
        let _lock = self.lock.lock();
        callback(&self.private)
    }

    /// Safely access the private field with a callback
    pub fn access_private_mut<F, A>(&mut self, callback: F) -> A
    where
        F: FnOnce(&mut T) -> A,
    {
        let _lock = self.lock.lock();
        callback(&mut self.private)
    }

    /// Get the immutable reference of the private field and assert it has type `P`.
    /// If the type of private field is not `P`, returns None.
    ///
    /// This API is marked as unsafe because user should acquire the read lock
    /// before calling `get_private`, or it will cause a concurrency problem.
    pub unsafe fn get_private<P: Any + 'static>(&self) -> Option<&P> {
        match TypeId::of::<P>() == self.type_id {
            true => Some(unsafe { core::mem::transmute(&self.private) }),
            false => None,
        }
    }

    /// Get the immutable reference of the private field without type assert.
    ///
    /// This API is marked as unsafe because:
    ///
    /// - user should ensure that the type conversion is valid.
    /// - user should acquire the read lock before calling `get_private`, or it will cause
    ///  a concurrency problem.
    pub unsafe fn get_private_unchecked(&self) -> &T {
        &self.private
    }

    /// Get the mutable reference of private field.
    ///
    /// This API is marked as unsafe because user should acquire the write lock
    /// before calling `get_private_mut`, or it will cause a concurrency problem.
    pub unsafe fn get_private_mut<P: Any + 'static>(&mut self) -> Option<&mut P> {
        match TypeId::of::<P>() == self.type_id {
            true => {
                // SAFETY: Safe. The type ID of P is equals to self.type_id,
                // thus the casting is safe.
                Some(unsafe { core::mem::transmute(&mut self.private) })
            }
            false => None,
        }
    }

    /// Get the mutable reference of the private field without type assert.
    ///
    /// This API is marked as unsafe because:
    ///
    /// - user should ensure that the type conversion is valid.
    /// - user should acquire the write lock before calling `get_private`, or it will cause
    ///  a concurrency problem.
    pub unsafe fn get_private_mut_unchecked(&mut self) -> &mut T {
        &mut self.private
    }

    /// Set the private field. `P` is the previous type of current private field.
    ///
    /// This API is marked as unsafe because users should ensure that they passed the
    /// valid type `T` for `set_private`, or it will lead to memory safety problem.
    pub unsafe fn set_private(&mut self, private: T) {
        let _lock = self.lock.lock();

        self.private = private;
        self.type_id = TypeId::of::<T>();
    }
}

/// Convert `unsigned argc` and `char** argv` into `Vec<&'static CStr>`
pub fn args_to_vec(argc: u32, argv: *mut *mut c_types::c_char) -> Result<Vec<&'static CStr>> {
    let iter = (0..argc)
        .map(|it| unsafe { *argv.add(it as usize) })
        .map(|ptr| unsafe { CStr::from_char_ptr(ptr) });

    let mut vec = Vec::try_with_capacity(argc as usize)?;
    for item in iter {
        vec.try_push(item)?;
    }

    Ok(vec)
}
