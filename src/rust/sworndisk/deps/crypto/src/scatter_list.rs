//! Rust wrapper of Linux scatter list data structure

use crate::prelude::*;

/// Memory scatter list
pub struct ScatterList<const N: usize> {
    list: [bindings::scatterlist; N],
}

impl<const N: usize> ScatterList<{ N }> {
    /// Create a scatterlist with N entries
    pub fn new() -> Result<Self> {
        let mut list = [bindings::scatterlist::default(); N];

        // SAFETY: Calling FFI function
        unsafe { bindings::sg_init_table(&mut list as *mut bindings::scatterlist, N as u32) };

        Ok(Self { list })
    }

    /// Create a scatter list with exact 1 entry
    pub fn new_one(buf: &mut Vec<u8>, buflen: usize) -> Result<Self> {
        let mut list = [bindings::scatterlist::default(); N];

        // SAFETY: Calling FFI function
        unsafe {
            bindings::sg_init_one(
                &mut list as *mut bindings::scatterlist,
                buf.as_mut_ptr() as *const c_void,
                buflen as u32,
            );
        };

        Ok(Self { list })
    }

    /// Set the buf for `index`-th entry.
    pub fn set_buf(&mut self, index: usize, buf: &mut Vec<u8>, buflen: usize) -> Result {
        if index >= N {
            return Err(EINVAL);
        }

        unsafe {
            bindings::sg_set_buf(
                &mut self.list[index] as *mut bindings::scatterlist,
                buf.as_mut_ptr() as *const c_void,
                buflen as u32,
            );
        };

        Ok(())
    }

    /// Set the buf for `index`-th entry.
    pub fn set_buf_slice(&mut self, index: usize, buf: &mut [u8], buflen: usize) -> Result {
        if index >= N {
            return Err(EINVAL);
        }

        unsafe {
            bindings::sg_set_buf(
                &mut self.list[index] as *mut bindings::scatterlist,
                buf as *const _ as *const c_void,
                buflen as u32,
            );
        };

        Ok(())
    }

    /// Get raw pointer reference of scatterlist
    pub fn raw(&self) -> *const bindings::scatterlist {
        &self.list as *const bindings::scatterlist
    }

    /// Get raw & mutable pointer reference of scatterlist
    pub fn raw_mut(&mut self) -> *mut bindings::scatterlist {
        &mut self.list as *mut bindings::scatterlist
    }
}

// TODO: free scatterlist in Drop?
