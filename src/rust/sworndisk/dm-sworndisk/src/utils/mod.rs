use crate::prelude::*;

pub mod bitmap;
pub mod debug_ignore;
pub mod linked_list;
pub mod lru;
pub mod traits;

pub use bitmap::*;
pub use debug_ignore::*;
pub use linked_list::*;
pub use lru::*;
pub use traits::*;

/// Get current system timestamp
#[inline]
pub fn current_timestamp() -> u64 {
    unsafe { bindings::ktime_get_ns() }
}

/// Translate the (sector, length) to block range [begin_lba, end_lba)
///
/// Returns (begin_lba, end_lba, begin_offset, end_offset).
///
/// ```
///      begin_offset                             end_offset
///       |-----|                                  |------|
///       |- - - * * *|* * * * * *|* * * * * *|* * - - - -| -
///        ^                                                ^
///   begin_lba                                         end_lba
/// ```
#[inline]
pub fn get_lba_range(begin_sector: u64, len: u64) -> (usize, usize, usize, usize) {
    let begin_byte = begin_sector * SECTOR_SIZE;
    let end_byte = begin_byte + len;
    let begin_lba = begin_byte / BLOCK_SIZE;
    let end_lba = if end_byte > 0 {
        (end_byte - 1) / BLOCK_SIZE + 1
    } else {
        1
    };
    let begin_offset = begin_byte - begin_lba * BLOCK_SIZE;
    let end_offset = end_lba * BLOCK_SIZE - end_byte;

    (
        begin_lba as usize,
        end_lba as usize,
        begin_offset as usize,
        end_offset as usize,
    )
}

/// Translate a sector address to block address
#[inline]
pub fn sector_to_block_address(sector: u64) -> u64 {
    sector / BLOCK_SECTORS
}

/// Translate a vector to a slide of same length
pub fn vec_to_slice<const N: usize>(vec: &Vec<u8>) -> Result<[u8; N]> {
    if vec.len() < N {
        return Err(EINVAL);
    }

    let mut slice = [0; N];
    slice.copy_from_slice(&vec[..]);

    Ok(slice)
}

/// Translate a slice to a vector of same length
pub fn slice_to_vec<const N: usize>(slice: &[u8; N]) -> Result<Vec<u8>> {
    let mut vec = Vec::new();
    vec.try_extend_from_slice(slice)?;

    Ok(vec)
}
