use crate::{
    prelude::*,
    regions::{checkpoint::Checkpoint, MemTable, Record},
    utils::vec_to_slice,
};

use crypto::get_random_bytes;

use kernel::rbtree::RBTree;

/// SwornDisk Data Segment
pub struct DataSegment {
    /// Data Segment buffer
    pub buffer: Vec<u8>,
    /// Hardware Block Address of current data segment
    ///
    /// Note that in Linux device mapper, we use "sector address" rather than
    /// "block address" to represent the `hba` field.
    pub hba: u64,
    /// Used blocks of current buffer
    pub used: u64,
    /// Map the logical block address (LBA) to the buffer position. This is essential
    /// for fragment write request.
    pub lba_index_map: RBTree<u64, usize>,
}

impl Debug for DataSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataSegment")
            .field("hba", &self.hba)
            .field("used", &self.used)
            .field("buffer", &self.buffer)
            .finish()
    }
}

impl DataSegment {
    /// Create a new data segment
    pub fn new(hba: u64) -> Result<Self> {
        let mut buffer = Vec::try_with_capacity(SEGMENT_SIZE as usize)?;
        buffer.try_resize(SEGMENT_SIZE as usize, 0u8)?;

        let lba_index_map = RBTree::new();

        Ok(DataSegment {
            buffer,
            hba,
            lba_index_map,
            used: 0,
        })
    }

    pub fn read(&self, lba: u64, data: &mut [u8], offset: usize, len: usize) -> Option<usize> {
        if let Some(block_buf_index) = self.lba_index_map.get(&lba) {
            let buf_begin = *block_buf_index + offset;
            let buf_end = buf_begin + len;
            data.copy_from_slice(&self.buffer[buf_begin..buf_end]);

            return Some(len);
        }

        return None;
    }

    /// Write the data into the buffer
    pub fn write(
        &mut self,
        lba: u64,
        data: &[u8],
        offset: usize,
        len: usize,
        aead: &Pin<Box<Aead>>,
        checkpoint: &mut Checkpoint,
        client: &mut DmIoClient,
        memtable: &mut MemTable,
        bdev: &BlockDevice,
    ) -> Result<(usize, u64)> {
        // First, we check the requested LBA is in current data segment now. If the requested LBA
        // has already taken a block in the current segment buffer, we can update the block in-place
        // to deal with the bio request smaller than a block size and reduce space consumption.
        //
        // If the data corresponding to the LBA exists in the buffer (assume it is located at `i`),
        // we just place the data at [i + offset, i + offset + len).
        if let Some(block_buf_index) = self.lba_index_map.get(&lba) {
            let hba = self.hba + (*block_buf_index as u64) / BLOCK_SIZE * BLOCK_SECTORS;
            let buf_begin = *block_buf_index + offset;
            let buf_end = buf_begin + len;
            self.buffer[buf_begin..buf_end].copy_from_slice(data);

            Ok((*block_buf_index / BLOCK_SIZE as usize, hba))
        }
        // If the data not exists, we allocate a new block from DST and log the block.
        else {
            let current_data_segment = checkpoint.current_data_segment;

            // Check current data segment has enough space to contain a block.
            if let Ok(block_index) = checkpoint.dst[current_data_segment].alloc_block() {
                // We calculate the range of current allocated block, and put the data in the proper area,
                // then update the LBA index and used counter.
                let hba = self.hba + (block_index as u64) * BLOCK_SECTORS;
                let block_buf_index = block_index * BLOCK_SIZE as usize;
                let buf_begin = block_buf_index + offset;
                let buf_end = buf_begin + len;

                self.buffer[buf_begin..buf_end].copy_from_slice(data);
                self.lba_index_map.try_insert(lba, block_buf_index)?;
                self.used += 1;

                Ok((block_index, hba))
            }
            // If there is no space left, then it's the time for us to alloc a new segment
            // and schedule a writeback task to write current segment to disk.
            else {
                Self::do_flush(
                    &mut self.buffer,
                    &mut self.lba_index_map,
                    &mut self.hba,
                    &mut self.used,
                    aead,
                    checkpoint,
                    client,
                    memtable,
                    bdev,
                )?;

                let current_data_segment = checkpoint.current_data_segment;
                let block_index = checkpoint.dst[current_data_segment].alloc_block()?;
                let hba = self.hba + (block_index as u64) * BLOCK_SECTORS;
                let block_buf_index = block_index * BLOCK_SIZE as usize;
                let buf_begin = block_buf_index + offset;
                let buf_end = buf_begin + len;

                self.buffer[buf_begin..buf_end].copy_from_slice(data);
                self.lba_index_map.try_insert(lba, block_buf_index)?;
                self.used += 1;

                Ok((block_index, hba))
            }
        }
    }

    /// Flush the data segment buffer
    pub fn flush(
        &mut self,
        aead: &Pin<Box<Aead>>,
        checkpoint: &mut Checkpoint,
        client: &mut DmIoClient,
        memtable: &mut MemTable,
        bdev: &BlockDevice,
    ) -> Result {
        Self::do_flush(
            &mut self.buffer,
            &mut self.lba_index_map,
            &mut self.hba,
            &mut self.used,
            aead,
            checkpoint,
            client,
            memtable,
            bdev,
        )
    }

    /// write current data segment into disk, and allocate a new segment
    fn do_flush(
        buffer: &mut Vec<u8>,
        lba_index_map: &mut RBTree<u64, usize>,
        hba: &mut u64,
        used: &mut u64,
        aead: &Pin<Box<Aead>>,
        checkpoint: &mut Checkpoint,
        client: &mut DmIoClient,
        memtable: &mut MemTable,
        bdev: &BlockDevice,
    ) -> Result {
        // generate random (key, nonce) and encrypt the data
        for (lba, index) in lba_index_map.iter() {
            let buf_begin = *index;
            let buf_end = buf_begin + BLOCK_SIZE as usize;

            let key = get_random_bytes(SWORNDISK_KEY_LENGTH)?;
            let mut nonce = get_random_bytes(SWORNDISK_NONCE_LENGTH)?;

            // SAFETY: Encrypt the data in-place. There is no concurrent access
            // of `self.buffer` so this is safe.
            let mac = unsafe {
                aead.as_ref().encrypt_in_place(
                    &key,
                    &mut nonce,
                    &mut buffer[buf_begin..buf_end],
                    BLOCK_SIZE as usize,
                )?
            };

            let record = Record {
                hba: *hba + *index as u64 / BLOCK_SIZE * BLOCK_SECTORS,
                key: vec_to_slice::<{ SWORNDISK_KEY_LENGTH }>(&key)?,
                nonce: vec_to_slice::<{ SWORNDISK_NONCE_LENGTH }>(&nonce)?,
                mac: vec_to_slice::<{ SWORNDISK_MAC_LENGTH }>(&mac)?,
            };

            memtable.insert(*lba, record)?;
        }

        // writeback
        let mut region = DmIoRegion::new(&bdev, *hba, SEGMENT_SECTORS)?;
        let mut io_req = DmIoRequest::with_kernel_memory(
            WRITE as i32, // req_op
            WRITE as i32, // req_op_flags
            buffer.as_mut_ptr() as *mut c_void,
            0, // offset
            client,
        );

        io_req.submit(&mut region);

        // allocate a new LBA->index map
        let mut new_map = RBTree::new();
        core::mem::swap(lba_index_map, &mut new_map);

        // allocate a new buffer and replace current buffer
        let mut new_buffer = Vec::try_with_capacity(SEGMENT_SIZE as usize)?;
        new_buffer.try_resize(SEGMENT_SIZE as usize, 0u8)?;
        core::mem::swap(buffer, &mut new_buffer);
        *used = 0;

        // allocate new data segment
        let current_data_segment = checkpoint.data_svt.alloc()?;
        checkpoint.current_data_segment = current_data_segment;
        *hba = (1 + current_data_segment as u64) * SEGMENT_SECTORS;

        Ok(())
    }
}
