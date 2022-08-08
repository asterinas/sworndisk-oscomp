// SPDX-License-Identifier: GPL-2.0

//! SwornDisk global context strucure

use crate::{
    prelude::*,
    regions::{
        Checkpoint, DataSegment, IndexSegment, IndirectBlock, LeafBlock, MemTable, SuperBlock, BIT,
    },
    utils::{DebugIgnore, LruCache},
    workers::{CompactionWorker, IoWorker},
};

use kernel::sync::{Mutex, RwSemaphore};

/// global SwornDisk context
pub static mut CONTEXT: Option<&mut SwornDiskContext> = None;

#[derive(Debug)]
#[repr(C)]
pub struct SwornDiskContext {
    /// AEAD (Authenticated Encryption with Associated Data) crypto handle
    pub aead: Pin<Box<Aead>>,
    /// BIO request queues pending to be handled
    pub bio_queue: DebugIgnore<Pin<Box<Mutex<LinkedList<Bio>>>>>,
    /// Device mapper block manager handle
    pub block_manager: DmBlockManager,
    /// SwornDisk checkpoint region
    pub checkpoint: Checkpoint,
    /// Device mapper I/O clinet
    pub dm_io_client: DmIoClient,
    /// data segment buffer
    pub data_seg_buffer: DataSegment,
    /// Real block device for storing data segment
    pub data_dev: DmDev,
    /// Index segment
    pub index_seg: IndexSegment,
    /// IndirectBlock LRU cache (HBA -> IndirectBlock)
    pub indirect_block_cache: LruCache<u64, IndirectBlock>,
    /// LeafBlock LRU cache (HBA -> LeafBlock)
    pub leaf_block_cache: LruCache<u64, LeafBlock>,
    /// Context MutexLock
    pub lock: DebugIgnore<Pin<Box<RwSemaphore<()>>>>,
    // / Real block device for storing meta info (superblocks, journal, checkpoint...)
    pub meta_dev: DmDev,
    /// Level 0 (in-memory) block index tree
    pub memtable: MemTable,
    /// start sector
    pub start: u64,
    /// SwornDisk superblock
    pub superblock: SuperBlock,
    /// Async work queue
    pub work_queue: Box<WorkQueue>,

    /// Worker for handle bio requests
    pub rw_worker: [WorkStruct; MAX_WORKERS],
    /// Worker for handle compaction
    pub compaction_worker: WorkStruct,
}

impl SwornDiskContext {
    /// Initialize async workers that will make use of any member in SwornDiskContext
    pub fn init_workers(&mut self) {
        for worker in &mut self.rw_worker {
            worker.init::<IoWorker>();
        }
        self.compaction_worker.init::<CompactionWorker>();
    }

    /// Flush SwornDisk
    pub fn flush(&mut self) -> Result {
        // flush data segment
        self.data_seg_buffer.flush(
            &self.aead,
            &mut self.checkpoint,
            &mut self.dm_io_client,
            &mut self.memtable,
            &self.data_dev.block_device()?,
        )?;

        // generate MemTable from BIT and write to index segment
        let bit = BIT::from_memtable(
            &mut self.memtable,
            &self.aead,
            &mut self.dm_io_client,
            &mut self.checkpoint,
            &self.meta_dev.block_device()?,
            &mut self.index_seg,
        )?;
        self.checkpoint.bit_category.add_bit(bit, 0)?;

        // write checkpoint
        let checkpoint_hba = self.superblock.checkpoint_region / SECTOR_SIZE;
        self.checkpoint.write_to_disk(
            &self.meta_dev.block_device()?,
            &mut self.dm_io_client,
            checkpoint_hba,
        )?;

        Ok(())
    }
}
