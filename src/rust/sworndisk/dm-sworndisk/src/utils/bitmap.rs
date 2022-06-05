//! BitMap implementation

use super::traits::{Deserialize, Serialize};
use crate::prelude::*;

/// Rust BitMap data structure
#[derive(Debug)]
#[repr(C)]
pub struct BitMap {
    is_seq: bool,
    max_len: usize,
    avail_len: usize,
    map: Vec<u8>,
}

type ItemType = u8;

/// Bits number an vector item contains
const BITMAP_ITEM_SIZE: usize = 8;

#[allow(dead_code)]
impl BitMap {
    /// Create a new BitMap with a capacity of `nr_bits` bits.
    pub fn new(nr_bits: usize) -> Result<Self> {
        // calculate the vector length to contain at least `nr_bits` bits.
        let should_extend = !(nr_bits % BITMAP_ITEM_SIZE == 0) as usize;
        let vec_len = nr_bits / BITMAP_ITEM_SIZE + should_extend;

        // create a vector of `vec_len` and fill all elements as 0
        let mut map = Vec::try_with_capacity(vec_len)?;
        map.try_resize(vec_len, 0)?;

        Ok(Self {
            map,
            is_seq: true,
            max_len: nr_bits,
            avail_len: nr_bits,
        })
    }

    /// Set the bit at `index` as `1`.
    pub fn set_bit(&mut self, index: usize) -> Result {
        if index >= self.max_len {
            return Err(EINVAL);
        }

        if self.avail_len <= 0 {
            return Err(ENOSPC);
        }

        if self.is_seq && index != self.max_len - self.avail_len {
            self.is_seq = false;
        }

        let item_index = index / BITMAP_ITEM_SIZE;
        let bit_index = index % BITMAP_ITEM_SIZE;

        // SAFETY: Safe. We have checked the index will not large than `self.max_len`,
        // which ensured that the index will not overflow, so it is OK to use `get_unchecked`
        // for greater performance.
        unsafe {
            let current = self.map.get_unchecked(item_index);
            *self.map.get_unchecked_mut(item_index) = current | (1 << bit_index);

            self.avail_len -= 1;
        };

        Ok(())
    }

    /// Set the bit at `index` as `0`.
    pub fn clear_bit(&mut self, index: usize) -> Result {
        if index >= self.max_len {
            return Err(EINVAL);
        }

        if self.is_seq && index != self.max_len - self.avail_len - 1 {
            self.is_seq = false;
        }

        let item_index = index / BITMAP_ITEM_SIZE;
        let bit_index = index % BITMAP_ITEM_SIZE;

        // SAFETY: Safe. We have checked the index will not large than `self.max_len`,
        // which ensured that the index will not overflow, so it is OK to use `get_unchecked`
        // for greater performance.
        unsafe {
            let current = self.map.get_unchecked(item_index);
            *self.map.get_unchecked_mut(item_index) = current & !(1 << bit_index);
            self.avail_len += 1;
        };

        Ok(())
    }

    /// Get the bit at `index`.
    pub fn get_bit(&self, index: usize) -> Result<bool> {
        if index >= self.max_len {
            return Err(EINVAL);
        }

        let item_index = index / BITMAP_ITEM_SIZE;
        let bit_index = index % BITMAP_ITEM_SIZE;

        // SAFETY: Safe. We have checked the index will not large than `self.max_len`,
        // which ensured that the index will not overflow, so it is OK to use `get_unchecked`
        // for greater performance.
        let result = unsafe { self.map.get_unchecked(item_index) & (1 << bit_index) };

        Ok(result > 0)
    }

    /// Get the first zero (unused) bit.
    pub fn get_first_zero_bit(&self) -> Result<usize> {
        if self.avail_len == 0 {
            return Err(ENOSPC);
        }

        if self.is_seq {
            return Ok(self.max_len - self.avail_len);
        }

        let full = !(0 as ItemType);
        for i in 0..self.map.len() {
            if self.map[i] == full {
                continue;
            }

            for bit in 0..BITMAP_ITEM_SIZE {
                if self.map[i] & (1 << bit) == 0 {
                    return Ok(i * BITMAP_ITEM_SIZE + bit);
                }
            }
        }

        Err(ENOSPC)
    }

    /// Check the current BitMap is full.
    pub fn is_full(&self) -> bool {
        self.avail_len == 0
    }

    /// Check the current BitMap is empty.
    pub fn is_empty(&self) -> bool {
        self.avail_len == self.max_len
    }

    /// Get the length of BitMap
    pub fn len(&self) -> usize {
        self.max_len
    }
}

impl Serialize for BitMap {
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();
        vec.try_extend_from_slice(&unsafe { mem::transmute::<bool, [u8; 1]>(self.is_seq) })?;
        vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(self.max_len) })?;
        vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(self.avail_len) })?;
        vec.try_extend_from_slice(&self.map)?;
        Ok(vec)
    }
}

impl Deserialize for BitMap {
    fn deserialize(buf: &[u8]) -> Result<Self> {
        let is_seq = unsafe { mem::transmute::<[u8; 1], bool>(buf[0..1].try_into().unwrap()) };
        let max_len = unsafe { mem::transmute::<[u8; 8], usize>(buf[1..9].try_into().unwrap()) };
        let avail_len = unsafe { mem::transmute::<[u8; 8], usize>(buf[9..17].try_into().unwrap()) };

        let should_extend = !(max_len % BITMAP_ITEM_SIZE == 0) as usize;
        let vec_len = max_len / BITMAP_ITEM_SIZE + should_extend;

        if vec_len + 17 != buf.len() {
            return Err(EINVAL);
        }

        let mut map = Vec::new();
        map.try_extend_from_slice(&buf[17..17 + vec_len])?;

        Ok(Self {
            is_seq,
            max_len,
            avail_len,
            map,
        })
    }
}
