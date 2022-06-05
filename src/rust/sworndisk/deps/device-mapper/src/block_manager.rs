use super::{block::BlockDevice, block_validator::DmBlockValidator, prelude::*};

/// Rust wrapper for `struct dm_block`. This is an empty struct.
///
/// # Invariant
///
/// The pointer `DmBlock::inner` is non-null and valid.
pub struct DmBlock {
    inner: *mut bindings::dm_block,
}

impl DmBlock {
    /// Create a DmBlock wrapper from raw pointer
    pub fn from_ptr(ptr: *mut bindings::dm_block) -> Result<Self> {
        match ptr.is_null() {
            true => Err(EINVAL),
            false => Ok(Self { inner: ptr }),
        }
    }

    /// Create an empty container of `struct dm_block`.
    pub unsafe fn new_uninit() -> Self {
        Self {
            inner: core::ptr::null_mut(),
        }
    }

    /// Get the raw pointer reference of `self.inner`.
    pub unsafe fn raw(&self) -> *mut bindings::dm_block {
        self.inner
    }

    /// Get the mutable raw pointer reference of `self.inner`
    pub unsafe fn raw_mut(&mut self) -> &mut *mut bindings::dm_block {
        &mut self.inner
    }

    /// Get block location
    pub fn location(&self) -> u64 {
        unsafe { bindings::dm_block_location(self.inner) }
    }

    /// Retrieve the data from current DmBlock and create a map to type T.
    ///
    /// # Safety
    ///
    /// Users should guarantee that the block contains a valid data of type `T`.
    pub unsafe fn data<T>(&self) -> *mut T {
        unsafe { core::mem::transmute(bindings::dm_block_data(self.inner)) }
    }

    /// Calculate the checksum range from [ptr, ptr + len)
    ///
    /// # Safety
    ///
    /// User should guarantee that `ptr` is valid and `ptr + len` will not overflow.
    pub unsafe fn checksum(ptr: *const c_types::c_void, len: usize, xor: u32) -> u32 {
        unsafe { bindings::dm_bm_checksum(ptr, len, xor) }
    }
}

impl Drop for DmBlock {
    fn drop(&mut self) {
        // SAFETY: Calling FFI function
        unsafe { bindings::dm_bm_unlock(self.inner) };
    }
}

/// Rust wrapper for `struct dm_block_manager`
///
/// # Invariant
///
/// The pointer `DmBlockManager::inner` is non-null and valid.
#[derive(Debug)]
pub struct DmBlockManager {
    inner: *mut bindings::dm_block_manager,
}

impl DmBlockManager {
    /// Create a `struct dm_block_manager` instance
    pub fn new(
        block_device: BlockDevice,
        block_size: u32,
        max_held_per_thread: u32,
    ) -> Result<Self> {
        // SAFETY: Calling FFI function
        let inner = unsafe {
            bindings::dm_block_manager_create(
                block_device.raw().ok_or(EINVAL)?,
                block_size,
                max_held_per_thread,
            )
        };

        Ok(Self { inner })
    }

    /// Acquire the read lock of block at location `location`. Return a `DmBlock` interface.
    /// When `DmBlock` is dropped, the lock is released automatically.
    pub fn read_lock(
        &self,
        location: u64,
        validator: Option<&DmBlockValidator>,
    ) -> Result<DmBlock> {
        let bm = self.inner;
        let validator = match validator {
            Some(validator) => unsafe { validator.raw() },
            None => core::ptr::null_mut(),
        };
        // SAFETY: Safe. `Dmblock::inner` is assigned below.
        let mut block = unsafe { DmBlock::new_uninit() };

        // SAFETY: Safe. We acquire the read lock at `location` and assign `DmBlock::inner`.
        let r = unsafe { bindings::dm_bm_read_lock(bm, location, validator, block.raw_mut()) };

        match r {
            0 => Ok(block),
            _ => Err(ENODATA),
        }
    }

    /// Acquire the write lock of block at location `location`. Return a `DmBlock` interface.
    /// When `DmBlock` is dropped, the lock is released automatically.
    ///
    /// # Safety
    ///
    /// Users should manually call `flush` to release the write lock and commit the change.
    pub unsafe fn write_lock(
        &self,
        location: u64,
        validator: Option<&DmBlockValidator>,
    ) -> Result<DmBlock> {
        let bm = self.inner;
        let validator = match validator {
            Some(validator) => unsafe { validator.raw() },
            None => core::ptr::null_mut(),
        };
        // SAFETY: Safe. `Dmblock::inner` is assigned below.
        let mut block = unsafe { DmBlock::new_uninit() };

        // SAFETY: Safe. We acquire the read lock at `location` and assign `DmBlock::inner`.
        let r = unsafe { bindings::dm_bm_write_lock(bm, location, validator, block.raw_mut()) };

        match r {
            0 => Ok(block),
            _ => Err(ENODATA),
        }
    }

    /// Flush the block manager and commit changes.
    pub fn flush(&self) -> i32 {
        unsafe { bindings::dm_bm_flush(self.inner) }
    }

    /// Get the block size
    pub fn block_size(&self) -> u32 {
        // SAFETY: Safe. `self.inner` is non-null and valid.
        unsafe { bindings::dm_bm_block_size(self.inner) }
    }
}

impl Drop for DmBlockManager {
    fn drop(&mut self) {
        // SAFETY: Safe. The type invariants guarantee that `inner` is non-null and valid.
        unsafe {
            bindings::dm_block_manager_destroy(self.inner);
        };
    }
}
