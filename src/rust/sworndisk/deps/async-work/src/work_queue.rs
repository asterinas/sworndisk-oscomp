use crate::prelude::*;
use core::{fmt, fmt::Debug};

/// Rust wrapper for `struct workqueue_struct`
#[repr(C)]
#[derive(Debug)]
pub struct WorkQueue(*mut bindings::workqueue_struct);

impl WorkQueue {
    /// Create a Rust wrapper for `struct workqueue_struct`
    pub fn new(name: &'static CStr, flags: u32, max_active: i32) -> Result<Box<Self>> {
        // SAFETY: Calling FFI function
        let workqueue = unsafe { bindings::alloc_workqueue(name.as_char_ptr(), flags, max_active) };

        let boxed = Box::try_new(Self(workqueue))?;
        Ok(boxed)
    }

    /// Push a work to the work queue
    pub fn queue_work(&self, work: &mut WorkStruct) {
        // SAFETY:
        // - `struct workqueue_struct*` is allocated in `WorkQueue::new` and is valid
        //   during the lifetime of `WorkStruct`.
        // - `struct work_struct*` is non-null and valid.
        unsafe { bindings::queue_work(self.0, work.raw()) };
    }
}

impl Drop for WorkQueue {
    fn drop(&mut self) {
        // SAFETY: `self.0` is allocated in `WorkQueue::new`.
        unsafe { bindings::destroy_workqueue(self.0) };
    }
}

/// Rust wrapper for `struct work_struct`
#[repr(C)]
pub struct WorkStruct(bindings::work_struct);

impl Debug for WorkStruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkStruct").finish()
    }
}

impl WorkStruct {
    /// Create a new `struct work_struct`
    pub fn new() -> Self {
        Self(bindings::work_struct::default())
    }

    /// Initialize
    pub fn init<T: WorkFuncTrait>(&mut self) {
        let work_func = unsafe { WorkFuncVTable::<T>::build().work };
        unsafe { bindings::init_work(&mut self.0, work_func) };
    }

    /// Get the raw reference of self.inner
    pub unsafe fn raw(&mut self) -> *mut bindings::work_struct {
        &mut self.0
    }
}

/// Async worker callback function
#[allow(unused_variables)]
pub trait WorkFuncTrait {
    /// `work_struct->func`
    fn work(work_struct: *mut bindings::work_struct) -> Result {
        unimplemented!()
    }
}

struct WorkFuncCallbacks {
    work: bindings::work_func_t,
}

struct WorkFuncVTable<T>(marker::PhantomData<T>);

impl<T: WorkFuncTrait> WorkFuncVTable<T> {
    unsafe extern "C" fn work(work_struct: *mut bindings::work_struct) {
        match T::work(work_struct) {
            Ok(()) => {}
            Err(e) => pr_warn!("error @ work_queue: {:?}", e),
        }
    }

    const VTABLE: WorkFuncCallbacks = WorkFuncCallbacks {
        work: Some(Self::work),
    };

    /// build the vtable
    pub const unsafe fn build() -> WorkFuncCallbacks {
        Self::VTABLE
    }
}
