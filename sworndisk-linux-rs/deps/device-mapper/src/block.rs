use core::{cmp, ptr};

use kernel::{
    bindings,
    error::{code::*, Result},
};

use super::{consts::Direction, prelude::*};

use crate::impl_getset;

/// A Rust wrapper for `struct block_device`
///
/// # Invariant
///
/// The pointer `BlockDevice::inner` is non-null and valid. Its reference count is also non-zero.
pub struct BlockDevice {
    inner: *mut bindings::block_device,
}

impl BlockDevice {
    impl_getset!(bd_start_sect, set_bd_start_sect, u64);
    impl_getset!(bd_nr_sectors, set_bd_nr_sectors, u64);
    impl_getset!(bd_stamp, stamp, set_stamp, u64);
    impl_getset!(bd_read_only, read_only, set_read_only, bool);
    impl_getset!(bd_dev, dev, set_dev, u32);
    impl_getset!(bd_openers, openers, set_openers, i32);
    impl_getset!(bd_holders, holders, set_holders, i32);
    impl_getset!(bd_write_holder, write_holder, set_write_holder, bool);
    impl_getset!(bd_partno, partno, set_partno, u8);
    impl_getset!(bd_fsfreeze_count, fsfreeze_count, set_fsfreeze_count, i32);

    /// Get raw pointer reference of `struct block_device`
    pub fn raw(&self) -> Option<*mut bindings::block_device> {
        match self.inner.is_null() {
            true => None,
            false => Some(self.inner),
        }
    }
}

impl From<*mut bindings::block_device> for BlockDevice {
    fn from(inner: *mut bindings::block_device) -> Self {
        Self { inner }
    }
}

/// A Rust wrapper for `struct bio`
///
/// # Invariant
///
/// `Bio::inner` should be non-null and valid.
pub struct Bio {
    inner: *mut bindings::bio,
}

impl From<*mut bindings::bio> for Bio {
    fn from(inner: *mut bindings::bio) -> Self {
        // increase the reference count of bio
        // SAFETY: `bio_put` is called in `drop`.
        unsafe { bindings::bio_get(inner) };

        Self { inner }
    }
}

impl Clone for Bio {
    fn clone(&self) -> Self {
        if self.inner.is_null() {
            return Self { inner: self.inner };
        }
        let cloned = unsafe {
            bindings::bio_clone_fast(
                self.inner,
                bindings::BINDINGS_GFP_NOIO,
                &mut bindings::fs_bio_set,
            )
        };
        Self { inner: cloned }
    }
}

impl Bio {
    impl_getset!(bi_opf, op_flags, set_op_flags, u32);
    impl_getset!(bi_flags, flags, set_flags, u16);
    impl_getset!(bi_ioprio, ioprio, set_ioprio, u16);
    impl_getset!(bi_write_hint, write_hint, set_write_hint, u16);
    impl_getset!(bi_status, status, set_status, u8);

    /// Convert a `struct bio*` to `Bio` wrapper.
    pub fn from_ptr(inner: *mut bindings::bio) -> Option<Self> {
        match inner.is_null() {
            true => None,
            false => Some(Self { inner }),
        }
    }

    /// Get next bio struct's wrapper
    pub fn next(&self) -> Option<Self> {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        Bio::from_ptr(unsafe { (*(self.inner)).bi_next })
    }

    /// Set the associated block device for Bio
    pub fn set_dev(&mut self, dev: &BlockDevice) -> Result {
        let bio = self.inner;
        let bdev = dev.raw().ok_or(EINVAL)?;

        // SAFETY: Safe. `inner` and `bdev` is non-null
        unsafe {
            bindings::bio_set_dev(bio, bdev);
            Ok(())
        }
    }

    /// Get sector index of bio iterator
    pub fn sector(&self) -> u64 {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { (*(self.inner)).bi_iter.bi_sector }
    }

    /// Set the new sector index of current bio
    pub fn set_sector(&mut self, sector: u64) {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { (*(self.inner)).bi_iter.bi_sector = sector };
    }

    /// Get the sector number of bio
    pub fn sectors(&self) -> u32 {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { (*(self.inner)).bi_iter.bi_size >> bindings::SECTOR_SHIFT }
    }

    /// Get the size of bio
    pub fn size(&self) -> u32 {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { (*(self.inner)).bi_iter.bi_size }
    }

    /// Set `bio.bi_iter.bi_size`
    pub fn set_size(&mut self, size: u32) {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { (*(self.inner)).bi_iter.bi_size = size }
    }

    /// Bio operation direction (READ, WRITE)
    pub fn direction(&self) -> Direction {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        let dir = unsafe { bindings::bio_data_dir(self.inner) };
        Direction::from(dir)
    }

    /// Get raw pointer reference of `struct bio`
    pub unsafe fn raw(&self) -> *mut bindings::bio {
        self.inner
    }

    /// set bi_flags bit
    pub fn set_flag(&mut self, flag: u32) {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { bindings::bio_set_flag(self.inner, flag) };
    }

    /// clear bi_flags bit
    pub fn clear_flag(&mut self, flag: u32) {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { bindings::bio_clear_flag(self.inner, flag) };
    }

    /// bio operation flags
    pub fn operation(&self) -> u32 {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { (*(self.inner)).bi_opf & bindings::REQ_OP_MASK }
    }

    /// Check if the bio carries data
    pub fn has_data(&self) -> bool {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { bindings::bio_has_data(self.inner) }
    }

    /// Get number of bytes
    pub fn len(&self) -> u32 {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { (*(*(self.inner)).bi_io_vec).bv_len }
    }

    /// Get bio_offset
    pub fn offset(&self) -> u32 {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { bindings::bio_offset(self.inner) }
    }

    /// informs the dm that the target only wants to process n sectors
    pub fn accept_partial(&self, nr_sectors: usize) {
        // SAFETY: From the type invariant, we can guarantee that `self.inner` is valid and non-null.
        unsafe { bindings::dm_accept_partial_bio(self.inner, nr_sectors as u32) };
    }

    /// Split a smaller bio from current bio
    pub fn split(&self, sectors: usize) -> Result<Bio> {
        unsafe {
            let split = bindings::bio_split(
                self.inner,
                sectors as i32,
                bindings::BINDINGS_GFP_NOIO,
                &mut bindings::fs_bio_set,
            );

            if split.is_null() {
                return Err(EINVAL);
            }

            // splitted & cloned bio should not increase the reference count
            Ok(Bio { inner: split })
        }
    }

    /// End a bio
    ///
    /// # Safety
    ///
    /// Calls this method to end a bio request must own this bio.
    pub unsafe fn end(&self) {
        unsafe { bindings::bio_endio(self.inner) };
    }

    /// Read data from sector. Returns (buffer, read-bytes)
    pub fn data(&self, max_len: usize) -> Result<(Vec<u8>, usize)> {
        // if `max_len` is specified, read up to `max_len` bytes
        let max_read_size = if max_len == 0 {
            self.size() as usize
        } else {
            max_len
        };

        let cloned = self.clone();
        let mut offset = 0;
        let mut buf = Vec::try_with_capacity(max_read_size)?;
        buf.try_resize(max_read_size, 0)?;

        loop {
            // process exact one sector every time
            let sectors = cloned.sectors();
            let split = if sectors == 1 {
                cloned.clone()
            } else {
                cloned.split(1)?
            };

            // calculate the maximum read length
            let page = unsafe { bindings::bio_page(split.raw()) };
            let read_len = cmp::min(split.size() as usize, max_read_size - offset);

            if read_len == 0 {
                break;
            }

            // copy data from page to buffer
            let slice = &mut buf[offset..offset + read_len];

            // SAFETY: now we are going to map page to a virtual address.
            // This is safe because the buffer slice is valid to read `read_len` bytes.
            unsafe {
                let page_addr = bindings::kmap_local_page(page).add(split.offset() as usize);

                offset = offset + read_len;
                ptr::copy(page_addr as *const u8, slice.as_mut_ptr(), read_len);

                bindings::kunmap_local(page_addr);
            };

            // final sector, no need to read other
            if sectors == 1 {
                break;
            }
        }

        Ok((buf, offset))
    }

    /// Write data to the bio. Returns the length of successfully written.
    pub fn set_data(&mut self, buf: Vec<u8>) -> Result<usize> {
        let max_write_len = buf.len();
        let cloned = self.clone();
        let mut offset = 0;

        loop {
            // process exact one sector every time
            let sectors = cloned.sectors();
            let split = if sectors == 1 {
                cloned.clone()
            } else {
                cloned.split(1)?
            };

            let page = unsafe { bindings::bio_page(split.raw()) };
            let write_len = cmp::min(split.size() as usize, max_write_len - offset);
            if write_len == 0 {
                break;
            }

            // copy data from page to buffer
            let slice = &buf[offset..offset + write_len];

            // SAFETY: now we are going to map page to a virtual address.
            // This is safe because the buffer slice is valid to read `read_len` bytes.
            unsafe {
                let page_addr = bindings::kmap_local_page(page).add(split.offset() as usize);

                ptr::copy(slice.as_ptr(), page_addr as *mut u8, write_len);
                offset = offset + write_len;

                bindings::kunmap_local(page_addr);
            };

            // final sector or no more data
            if sectors == 1 || offset == max_write_len {
                break;
            }
        }

        Ok(offset)
    }
}

impl Drop for Bio {
    fn drop(&mut self) {
        // SAFETY: decrease the reference count of bio.
        // The reference count has been added in `Bio::from`.
        unsafe { bindings::bio_put(self.inner) };
    }
}
