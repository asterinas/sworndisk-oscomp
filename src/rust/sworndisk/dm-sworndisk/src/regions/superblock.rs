use crate::prelude::*;

/// SwornDisk superblock. For robustness, there are two copies of superblock,
/// located in block index 0 and 1.
///
/// Note that the superblock of SwornDisk is read-only.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct SuperBlock {
    /// Checksum of superblock
    pub checksum: u64,
    /// Magic number
    pub magic_number: u64,

    /// number of blocks in data regions
    pub nr_blocks: u64,
    /// number of data segments
    pub nr_data_segments: u64,
    /// number of index segment
    pub nr_index_segments: u64,
    /// per block size (unit: Byte)
    pub block_size: u64,
    /// per segment size (unit: Byte)
    pub segment_size: u64,
    /// journal region size (unit: Byte)
    pub journal_size: u64,

    /// offset of the index region (byte)
    pub index_region: u64,
    /// offset of the journal region (byte)
    pub journal_region: u64,
    /// offset of the checkpoint region (byte)
    pub checkpoint_region: u64,
}

/// The size of SuperBlock struct
pub const SWORNDISK_SUPERBLOCK_SIZE: usize = mem::size_of::<SuperBlock>();

impl SuperBlock {
    /// Create a new SuperBlock
    pub fn new(data_nbytes: u64, index_nbytes: u64, journal_nbytes: u64) -> Self {
        // floor the block number and segment number
        let nr_blocks = data_nbytes / BLOCK_SIZE;
        let nr_data_segments = nr_blocks / SEGMENT_BLOCK_NUMBER;
        let nr_index_segments =
            index_nbytes / SEGMENT_SIZE + (index_nbytes % SEGMENT_SIZE == 0) as u64;

        let index_region = SEGMENT_SIZE;
        let journal_region = index_region + nr_index_segments * SEGMENT_SIZE;
        let checkpoint_region = journal_region + journal_nbytes;

        let mut superblock = SuperBlock {
            nr_blocks,
            nr_data_segments,
            nr_index_segments,
            index_region,
            journal_region,
            checkpoint_region,

            checksum: 0,
            magic_number: SWORNDISK_MAGIC_NUMBER,
            block_size: BLOCK_SIZE,
            segment_size: SEGMENT_SIZE,
            journal_size: journal_nbytes,
        };

        let checksum = superblock.checksum();
        superblock.checksum = checksum as u64;

        superblock
    }

    /// Validate the integrity of superblock
    pub fn validate(&self) -> bool {
        let checksum = self.checksum() as u64;
        checksum == self.checksum && SWORNDISK_MAGIC_NUMBER == self.magic_number
    }

    /// Calcuate the checksum of superblock
    pub fn checksum(&self) -> u32 {
        // SAFETY: TBD
        unsafe {
            DmBlock::checksum(
                &self.magic_number as *const _ as *const c_void,
                SWORNDISK_SUPERBLOCK_SIZE - 8, // ignore checksum
                SWORNDISK_SUPERBLOCK_XOR,
            )
        }
    }

    /// Read the superblock from disk
    pub fn read_from_disk(block_manager: &DmBlockManager) -> Result<Self> {
        let block = block_manager.read_lock(SWORNDISK_FIRST_SUPERBLOCK_LOCATION, None)?;
        // SAFETY: We will verify the block is a valid block or not after.
        let data = unsafe { block.data::<SuperBlock>() };
        if data.is_null() {
            return Err(EINVAL);
        }

        // SAFETY: We can guarantee that `data` is non-null, but is not always valid.
        // So we validate its integrity first. And if the data passes the validation,
        // we can guarantee that it has a proper memory alignment.
        unsafe {
            if (*data).validate() {
                return Ok(data.read());
            }
        };

        // If the first superblock is invalid, we check the second superblock.
        let block = block_manager.read_lock(SWORNDISK_SECOND_SUPERBLOCK_LOCATION, None)?;
        // SAFETY: We will verify the block is a valid block or not after.
        let data = unsafe { block.data::<SuperBlock>() };
        unsafe {
            if (*data).validate() {
                // TODO: fix the first superblock @kirainmoe
                return Ok(data.read());
            }
        };

        Err(EINVAL)
    }

    /// Write the superblock to disk
    pub fn write_to_disk(&self, block_manager: &DmBlockManager) -> Result {
        // SAFETY: Safe. `DmBlockManager::flush()` is called.
        unsafe {
            let block = block_manager.write_lock(SWORNDISK_FIRST_SUPERBLOCK_LOCATION, None)?;
            let data = block.data::<SuperBlock>();
            ptr::copy(self as *const SuperBlock, data, 1);
        };
        block_manager.flush();

        unsafe {
            let block = block_manager.write_lock(SWORNDISK_SECOND_SUPERBLOCK_LOCATION, None)?;
            let data = block.data::<SuperBlock>();
            ptr::copy(self as *const SuperBlock, data, 1);
        };
        block_manager.flush();

        Ok(())
    }

    /// Get the number of data segments
    pub fn data_segments_number(&self) -> u64 {
        self.nr_data_segments
    }

    /// Get the number of index segments
    pub fn index_segments_number(&self) -> u64 {
        self.nr_index_segments
    }
}

impl SuperBlock {
    fn get_checkpoint_size(nr_data_segments: u64, nr_index_segments: u64) -> u64 {
        let data_svt_size = if nr_data_segments % 8 == 0 {
            nr_data_segments / 8
        } else {
            nr_data_segments / 8 + 1
        }; // byte

        let index_svt_size = if nr_index_segments % 8 == 0 {
            nr_index_segments / 8
        } else {
            nr_index_segments / 8 + 1
        }; // byte

        let dst_size = nr_data_segments * SEGMENT_BLOCK_NUMBER / 8; // byte
        let params_size = 8;

        data_svt_size + index_svt_size + dst_size + params_size
    }
}
