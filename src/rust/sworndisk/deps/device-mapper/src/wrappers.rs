use super::{
    block::BlockDevice,
    callbacks::{DmCallbacks, DmCallbacksVTable},
    prelude::*,
    utils::PrivateField,
};
/// Rust wrapper for device mapper C types.
use core::fmt::Debug;

/// Rust wrapper for `struct target_type`: Information about a device mapper target type
pub struct TargetType {
    inner: Pin<Box<bindings::target_type>>,
}

impl TargetType {
    /// Create a `struct target_type`
    pub fn new(
        name: &'static CStr,
        version: [u32; 3],
        features: u64,
        module: &'static ThisModule,
    ) -> Result<Self> {
        let mut target_type = bindings::target_type::default();
        target_type.name = name.as_char_ptr();
        target_type.features = features;
        target_type.module = module.0;
        target_type.version = version;

        let boxed_target_type = Box::try_new(target_type)?;

        Ok(Self {
            inner: Pin::from(boxed_target_type),
        })
    }

    /// Create a pinned Rust wrapper of `struct target_type`
    pub fn new_pinned(
        name: &'static CStr,
        version: [u32; 3],
        features: u64,
        module: &'static ThisModule,
    ) -> Result<Pin<Box<Self>>> {
        let target_type = Self::new(name, version, features, module)?;

        Ok(Pin::from(Box::try_new(target_type)?))
    }

    /// Get const raw pointer reference of `self.inner`
    pub fn raw(self: Pin<&mut Self>) -> *const bindings::target_type {
        self.inner.as_ref().get_ref()
    }

    /// Get mutable raw pointer reference of `self.inner`
    pub fn raw_mut(mut self: Pin<&mut Self>) -> *mut bindings::target_type {
        // SAFETY: users must guarantee that you will never move the data
        // out of the mutable reference.
        unsafe { self.inner.as_mut().get_unchecked_mut() }
    }

    /// Register the target type to device mapper registry table
    pub fn register<T: DmCallbacks>(self: Pin<&mut Self>) -> i32 {
        // SAFETY: Safe. We will not move the data out of the reference.
        let this = unsafe { self.get_unchecked_mut() };
        // SAFETY: The adapter doesn't retrieve any state yet, so it's compatible with any
        // registration.
        let vtable = unsafe { DmCallbacksVTable::<T>::build() };

        let mut inner_mut = this.inner.as_mut();
        inner_mut.ctr = vtable.ctr;
        inner_mut.dtr = vtable.dtr;
        inner_mut.map = vtable.map;
        inner_mut.status = vtable.status;
        inner_mut.prepare_ioctl = vtable.prepare_ioctl;
        inner_mut.iterate_devices = vtable.iterate_devices;
        inner_mut.report_zones = vtable.report_zones;

        // SAFETY: users must guarantee that you will never move the data
        // out of the mutable reference.
        let target = unsafe { this.inner.as_mut().get_unchecked_mut() };
        unsafe { bindings::dm_register_target(target) }
    }
}

impl Drop for TargetType {
    fn drop(&mut self) {
        let target = unsafe { self.inner.as_mut().get_unchecked_mut() };
        unsafe { bindings::dm_unregister_target(target) }
    }
}

unsafe impl Sync for TargetType {}

/// Rust wrapper for `struct dm_dev`
#[derive(Clone, Debug)]
pub struct DmDev {
    owner: bool,
    inner: *mut bindings::dm_dev,
}

impl DmDev {
    /// Create a new rust wrapper for `struct dm_dev`
    pub fn new() -> Result<Self> {
        let dm_dev = bindings::dm_dev::default();
        let boxed = Box::try_new(dm_dev)?;
        let mut pinned = Pin::new(boxed);

        // SAFETY: safe. `dm_dev` is pinned.
        let self_ref = unsafe { pinned.as_mut().get_unchecked_mut() };

        Ok(Self {
            owner: true,
            inner: self_ref,
        })
    }

    /// Get raw pointer of `struct dm_dev`
    pub fn raw(&self) -> Option<*mut bindings::dm_dev> {
        match self.inner.is_null() {
            true => None,
            false => Some(self.inner),
        }
    }

    /// Get mutable refernce of `self.inner`
    pub fn inner_mut(&mut self) -> &mut *mut bindings::dm_dev {
        &mut self.inner
    }

    /// Get `bdev` of `dm_dev`
    pub fn block_device(&self) -> Result<BlockDevice> {
        let inner = self.inner;
        match inner.is_null() {
            true => Err(EINVAL),
            false => Ok(BlockDevice::from(unsafe { (*inner).bdev })),
        }
    }

    /// Set `bdev` of `dm_dev`
    pub fn set_block_device(&mut self, block_device: &BlockDevice) -> Result {
        let bdev = block_device.raw().ok_or(EINVAL)?;
        let inner = self.inner;
        match inner.is_null() {
            true => Err(EINVAL),
            false => unsafe {
                (*inner).bdev = bdev;
                Ok(())
            },
        }
    }
}

impl From<*mut bindings::dm_dev> for DmDev {
    fn from(target: *mut bindings::dm_dev) -> Self {
        Self {
            owner: false,
            inner: target,
        }
    }
}

impl Drop for DmDev {
    fn drop(&mut self) {
        if !self.inner.is_null() && self.owner {
            drop(&self.inner);
        }
    }
}

/// Rust wrapper for `struct dm_target`.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DmTarget {
    inner: *mut bindings::dm_target,
}

impl DmTarget {
    impl_getset!(begin, set_begin, u64);
    impl_getset!(len, set_len, u64);
    impl_getset!(max_io_len, set_max_io_len, u32);
    impl_getset!(num_flush_bios, set_num_flush_bios, u32);
    impl_getset!(num_discard_bios, set_num_discard_bios, u32);
    impl_getset!(num_secure_erase_bios, set_num_secure_erase_bios, u32);
    impl_getset!(num_write_same_bios, set_num_write_same_bios, u32);
    impl_getset!(num_write_zeroes_bios, set_num_write_zeroes_bios, u32);
    impl_getset!(per_io_data_size, set_per_io_data_size, u32);

    /// Get raw pointer reference of `dm_target`
    pub fn raw(&self) -> Option<*mut bindings::dm_target> {
        match self.inner.is_null() {
            true => None,
            false => Some(self.inner),
        }
    }

    /// Initialize RwLock and private field
    ///
    /// # Safety
    ///
    /// This API is marked as unsafe, because users should obey the following rules
    /// when calling `init_lock_and_private`:
    ///
    /// - Only calling this method in the constructor of `dm_target`
    /// - Call `drop_private_field()` in the destructor of `dm_target`, or it will lead to a memory leak
    pub unsafe fn init_lock_and_private<T: Any + Debug + 'static>(&mut self, target: T) -> Result {
        let inner = self.inner;
        if inner.is_null() {
            return Err(EINVAL);
        }

        let private = unsafe { (*inner).private };
        if private.is_null() {
            let field = PrivateField::new(target).unwrap();
            unsafe {
                (*inner).private = Box::into_raw(field) as *mut c_types::c_void;
            };
        }

        Ok(())
    }

    /// Set the `private` field of `dm_target`
    pub fn set_private<T: Any + Debug + 'static>(&mut self, target: T) -> Result {
        let inner = self.inner;
        if inner.is_null() {
            return Err(EINVAL);
        }

        // SAFETY: Safe, `inner` is non-null.
        let ptr = unsafe { (*inner).private };

        // If the `private` field is null, we can infer that the `struct dm_target` is
        // not managed by Rust. For safety reason we make this set_private request fail.
        if ptr.is_null() {
            return Err(EINVAL);
        }

        // Convert raw pointer to a Arc (Ref) to PrivateField
        let mut private = unsafe { Box::from_raw(ptr as *mut PrivateField<T>) };
        unsafe { private.set_private(target) };

        core::mem::forget(private);

        Ok(())
    }

    /// Drop the private field
    pub unsafe fn drop_private_field<T: Any + Debug + 'static>(&mut self) {
        if self.inner.is_null() {
            return;
        }

        // SAFETY: users should not modify the `private` directly.
        let private = unsafe { (*(self.inner)).private };
        if !private.is_null() {
            let _ = unsafe { Box::from_raw(private as *mut PrivateField<T>) };
        }
    }

    /// Access the private field in a callback
    pub fn access_private<T: Any + Debug + 'static, F, A>(&self, callback: F) -> Result<A>
    where
        F: FnOnce(&T) -> A,
    {
        let inner = self.inner;
        if inner.is_null() {
            return Err(EINVAL);
        }

        // SAFETY: Safe, `inner` is non-null.
        let ptr = unsafe { (*inner).private };

        // If the `private` field is null, we can infer that the `struct dm_target` is
        // not managed by Rust. For safety reason we make this set_private request fail.
        if ptr.is_null() {
            return Err(EINVAL);
        }

        // Convert raw pointer to a Box<PrivateField<T>>
        let private = unsafe { Box::from_raw(ptr as *mut PrivateField<T>) };
        let res = private.access_private(callback);

        core::mem::forget(private);

        Ok(res)
    }

    /// Access the private field in a callback
    pub fn access_private_mut<T: Any + Debug + 'static, F, A>(&self, callback: F) -> Result<A>
    where
        F: FnOnce(&mut T) -> A,
    {
        let inner = self.inner;
        if inner.is_null() {
            return Err(EINVAL);
        }

        // SAFETY: Safe, `inner` is non-null.
        let ptr = unsafe { (*inner).private };

        // If the `private` field is null, we can infer that the `struct dm_target` is
        // not managed by Rust. For safety reason we make this set_private request fail.
        if ptr.is_null() {
            return Err(EINVAL);
        }

        // Convert raw pointer to a Box<PrivateField<T>>
        let mut private = unsafe { Box::from_raw(ptr as *mut PrivateField<T>) };
        let res = private.access_private_mut(callback);

        core::mem::forget(private);

        Ok(res)
    }

    /// Lookup device and assign to dm_target.
    pub fn get_device(&mut self, path: &'static CStr, mode: u32, dev: &mut DmDev) -> i32 {
        let ti = self.raw().unwrap();
        let dev = dev.inner_mut();
        unsafe { bindings::dm_get_device(ti, path.as_char_ptr(), mode, dev) }
    }

    /// ...
    pub fn put_device(&self, dev: &DmDev) {
        let ti = self.raw().unwrap();
        let dev = dev.raw().unwrap();
        unsafe {
            bindings::dm_put_device(ti, dev);
        };
    }

    /// Get kernel module instance reference from `type.module`
    pub fn this_module(&self) -> ThisModule {
        // SAFETY: From the type invariant we can know `self.inner` is valid and non-null.
        unsafe { ThisModule::from_ptr((*(*(self.inner)).type_).module) }
    }
}

impl From<*mut bindings::dm_target> for DmTarget {
    fn from(inner: *mut bindings::dm_target) -> Self {
        Self { inner }
    }
}

/// Rust wrapper for `struct dm_report_zones_args`
pub struct DmReportZonesArgs {
    inner: *mut bindings::dm_report_zones_args,
}

impl From<*mut bindings::dm_report_zones_args> for DmReportZonesArgs {
    fn from(inner: *mut bindings::dm_report_zones_args) -> Self {
        Self { inner }
    }
}

impl DmReportZonesArgs {
    impl_getset!(start, set_start, u64);
    impl_getset!(next_sector, set_next_sector, u64);
    impl_getset!(zone_idx, set_zone_idx, u32);

    /// Get original data
    pub fn orig_data<T>(&self) -> Result<&mut T> {
        let inner = self.inner;
        match inner.is_null() {
            true => Err(EINVAL),
            false => Ok(unsafe { core::mem::transmute((*inner).orig_data) }),
        }
    }

    /// Get `dm_target` from `struct dm_report_zones_args`
    pub fn target(&self) -> Result<DmTarget> {
        let inner = self.inner;
        match inner.is_null() {
            true => Err(EINVAL),
            false => Ok(unsafe { DmTarget::from((*inner).tgt) }),
        }
    }
}
