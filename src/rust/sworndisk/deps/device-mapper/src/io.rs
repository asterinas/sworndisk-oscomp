//! Device Mapper low-level I/O

use crate::{block::BlockDevice, prelude::*};

/// Rust wrapper for `struct dm_io_region`
pub struct DmIoRegion(bindings::dm_io_region);

impl DmIoRegion {
    /// Create a `struct dm_io_region` wrapper, which will make an I/O request
    /// of length `count` sectors at the hardware sector position `sector`.
    pub fn new(bdev: &BlockDevice, sector: u64, count: u64) -> Result<Self> {
        Ok(Self(bindings::dm_io_region {
            sector,
            count,
            bdev: bdev.raw().ok_or(EINVAL)?,
        }))
    }

    /// Get the raw mutable pointer reference of `struct dm_io_region`
    pub unsafe fn raw(&mut self) -> *mut bindings::dm_io_region {
        &mut self.0 as *mut bindings::dm_io_region
    }
}

/// Rust wrapper for `struct dm_io_client`.
#[derive(Debug)]
pub struct DmIoClient(*mut bindings::dm_io_client);

impl DmIoClient {
    /// Create a `struct dm_io_client` wrapper
    pub fn new() -> Self {
        // SAFETY: Calling FFI function
        let client = unsafe { bindings::dm_io_client_create() };
        Self(client)
    }

    /// Get the raw mutable pointer reference of `struct dm_io_client`
    pub unsafe fn raw(&self) -> *mut bindings::dm_io_client {
        self.0
    }
}

impl Drop for DmIoClient {
    fn drop(&mut self) {
        // SAFETY: `self.0` is created in `DmIoClient::new`.
        unsafe { bindings::dm_io_client_destroy(self.0) };
    }
}

/// Rust wrapper for `struct dm_io_request`
pub struct DmIoRequest(bindings::dm_io_request);

impl DmIoRequest {
    /// Create a DM I/O request that associated with buffer.
    pub fn with_kernel_memory<'a>(
        req_op: i32,
        req_op_flags: i32,
        buffer: *mut c_types::c_void,
        offset: u32,
        client: &'a DmIoClient,
    ) -> Self {
        let request = bindings::dm_io_request {
            bi_op: req_op,
            bi_op_flags: req_op_flags,
            mem: bindings::dm_io_memory {
                type_: bindings::dm_io_mem_type_DM_IO_KMEM,
                offset: offset,
                ptr: bindings::dm_io_memory__bindgen_ty_1 {
                    addr: buffer as *mut _ as *mut c_types::c_void,
                },
            },
            notify: bindings::dm_io_notify {
                fn_: None,
                context: core::ptr::null_mut() as *mut c_types::c_void,
            },
            client: unsafe { client.raw() },
        };

        Self(request)
    }

    /// Submit a DM I/O request of 1 region. Returns `sync_error_bits`.
    pub fn submit(&mut self, region: &mut DmIoRegion) -> u64 {
        let mut sync_error_bits: u64 = 0;
        unsafe {
            bindings::dm_io(
                &mut self.0 as *mut bindings::dm_io_request,
                1,
                region.raw(),
                &mut sync_error_bits,
            )
        };
        sync_error_bits
    }
}
