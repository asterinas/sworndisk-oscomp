use super::prelude::*;

use super::{
    block::{Bio, BlockDevice},
    utils::args_to_vec,
    wrappers::{DmDev, DmReportZonesArgs, DmTarget},
};

/// Represents which fields of device mapper callbacks should be populated
#[derive(Debug)]
pub struct ToUse {
    /// use dm_ctr_fn
    pub ctr: bool,
    /// use dm_dtr_fn
    pub dtr: bool,
    /// use dm_map_fn
    pub map: bool,
    /// use dm_status_fn
    pub status: bool,
    /// use dm_prepare_ioctl_fn
    pub prepare_ioctl: bool,
    /// use dm_iterate_devices_fn
    pub iterate_devices: bool,
    /// use dm_report_zones fn
    pub report_zones: bool,
}

/// Default value for `ToUse`
pub const USE_NONE: ToUse = ToUse {
    ctr: false,
    dtr: false,
    map: false,
    status: false,
    prepare_ioctl: false,
    iterate_devices: false,
    report_zones: false,
};

/// Callbacks trait for Device Mapper interface (ctr, dtr, map...)
#[allow(unused_variables)]
pub trait DmCallbacks {
    /// The methods to use to populate device mapper callbacks
    const TO_USE: ToUse;

    /// constructor function
    fn ctr(target: DmTarget, args: Vec<&'static CStr>) -> Result<i32> {
        todo!()
    }

    /// destructor function
    fn dtr(target: DmTarget) -> Result {
        todo!()
    }

    /// mapper funnction
    fn map(target: DmTarget, bio: Bio) -> Result<i32> {
        todo!()
    }

    /// status callback function
    fn status(target: DmTarget, _type: u32, flags: u32) -> Result {
        todo!()
    }

    /// prepare ioctl callback function
    fn prepare_ioctl(target: DmTarget, bdev_setter: &dyn Fn(&BlockDevice)) -> Result<i32> {
        todo!()
    }

    /// iterate devices callback function
    fn iterate_devices(target: DmTarget) -> Result<(DmDev, u64, u64)> {
        todo!()
    }

    /// report zones callback function
    fn report_zones(target: DmTarget, args: DmReportZonesArgs, nr_zones: u32) -> Result<i32> {
        todo!()
    }
}

/// Callbacks vtable for `struct target_type`
pub struct TargetTypeCallbacks {
    /// C type of constructor function
    pub ctr: bindings::dm_ctr_fn,
    /// C type of destructor function
    pub dtr: bindings::dm_dtr_fn,
    /// C type of map function
    pub map: bindings::dm_map_fn,
    /// C type of status function
    pub status: bindings::dm_status_fn,
    /// C type of prepare_ioctl function
    pub prepare_ioctl: bindings::dm_prepare_ioctl_fn,
    /// C type of iterate_devices function
    pub iterate_devices: bindings::dm_iterate_devices_fn,
    /// C type of report_zones function
    pub report_zones: bindings::dm_report_zones_fn,
}

/// FFI functions table
pub struct DmCallbacksVTable<T>(marker::PhantomData<T>);

#[allow(unused_variables)]
impl<T: DmCallbacks> DmCallbacksVTable<T> {
    unsafe extern "C" fn ctr(
        target: *mut bindings::dm_target,
        argc: c_types::c_uint,
        argv: *mut *mut c_types::c_char,
    ) -> c_types::c_int {
        let target = DmTarget::from(target);
        let args = args_to_vec(argc, argv).unwrap();
        let res = T::ctr(target, args);

        match res {
            Ok(ret) => ret,
            Err(e) => e.to_kernel_errno(),
        }
    }

    unsafe extern "C" fn dtr(target: *mut bindings::dm_target) {
        let res = T::dtr(DmTarget::from(target));
        match res {
            Ok(()) => {}
            Err(e) => pr_warn!("error @ dtr: {:?}", e),
        };
    }

    unsafe extern "C" fn map(
        target: *mut bindings::dm_target,
        bio: *mut bindings::bio,
    ) -> c_types::c_int {
        let target = DmTarget::from(target);
        let bio = Bio::from(bio);

        let res = T::map(target, bio);
        match res {
            Ok(ret) => ret,
            Err(e) => e.to_kernel_errno(),
        }
    }

    unsafe extern "C" fn status(
        target: *mut bindings::dm_target,
        _type: bindings::status_type_t,
        status_flags: c_types::c_uint,
        result: *mut c_types::c_char,
        maxlen: c_types::c_uint,
    ) {
        let target = DmTarget::from(target);
        let res = T::status(target, _type, status_flags);

        match res {
            Ok(()) => {}
            Err(e) => pr_warn!("err @ status: {:?}", e),
        };
    }

    unsafe extern "C" fn prepare_ioctl(
        target: *mut bindings::dm_target,
        bdev: *mut *mut bindings::block_device,
    ) -> c_types::c_int {
        let bdev_setter = |block_device: &BlockDevice| {
            // SAFETY: `bdev` is non-null, deref is safe.
            unsafe {
                (*bdev) = block_device.raw().unwrap();
            };
        };

        let res = T::prepare_ioctl(DmTarget::from(target), &bdev_setter);
        match res {
            Ok(ret) => ret,
            Err(e) => e.to_kernel_errno(),
        }
    }

    unsafe extern "C" fn iterate_devices(
        ti: *mut bindings::dm_target,
        fn_: bindings::iterate_devices_callout_fn,
        data: *mut c_types::c_void,
    ) -> c_types::c_int {
        let res = T::iterate_devices(DmTarget::from(ti));
        match res {
            Ok((dev, start, len)) => {
                let dev = dev.raw().unwrap();

                // SAFETY: calling FFI functions
                unsafe { fn_.unwrap()(ti, dev, start, len, data) }
            }
            Err(e) => e.to_kernel_errno(),
        }
    }

    unsafe extern "C" fn report_zones(
        ti: *mut bindings::dm_target,
        args: *mut bindings::dm_report_zones_args,
        nr_zones: c_types::c_uint,
    ) -> c_types::c_int {
        let target = DmTarget::from(ti);
        let args = DmReportZonesArgs::from(args);
        let res = T::report_zones(target, args, nr_zones);
        match res {
            Ok(ret) => ret,
            Err(e) => e.to_kernel_errno(),
        }
    }

    const VTABLE: TargetTypeCallbacks = TargetTypeCallbacks {
        ctr: match T::TO_USE.ctr {
            true => Some(Self::ctr),
            false => None,
        },
        dtr: match T::TO_USE.dtr {
            true => Some(Self::dtr),
            false => None,
        },
        map: match T::TO_USE.map {
            true => Some(Self::map),
            false => None,
        },
        status: match T::TO_USE.status {
            true => Some(Self::status),
            false => None,
        },
        prepare_ioctl: match T::TO_USE.prepare_ioctl {
            true => Some(Self::prepare_ioctl),
            false => None,
        },
        iterate_devices: match T::TO_USE.iterate_devices {
            true => Some(Self::iterate_devices),
            false => None,
        },
        report_zones: match T::TO_USE.report_zones {
            true => Some(Self::report_zones),
            false => None,
        },
    };

    /// Build a vtable of device mapper callbacks
    pub const unsafe fn build() -> TargetTypeCallbacks {
        Self::VTABLE
    }
}
