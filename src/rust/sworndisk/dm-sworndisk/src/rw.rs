use crate::{
    context::SwornDiskContext,
    prelude::*,
    regions::{
        Checkpoint, DataSegment, IndexSegment, IndirectBlock, LeafBlock, MemTable, Record, BIT,
    },
    utils::{get_lba_range, slice_to_vec, LruCache},
};

pub struct RwWorker;

impl WorkFuncTrait for RwWorker {
    /// functions to handle block I/O request asynchronously
    fn work(work_struct: *mut bindings::work_struct) -> Result {
        // SAFETY:
        //
        // Here we want to get some members such as block device, data segment buffers and so on
        // from the global context `struct SwornDiskContext`. Since we have stored the worker
        // struct in the Context, we can caluclate the offset of `struct work_struct*` to get
        // the memory address of `struct SwornDiskContext` using the macro `container_of` provided
        // by Linux kernel. From the lifetime of the device mapper target, we can infer that the
        // pointer is valid and non-null.
        //
        // However, there can be some concurrency problems while accessing the SwornDiskContext
        // by the raw pointer directly, since there is no mutex guarantee for the raw pointer.
        // Thus, we should get the lock which protects the SwornDiskContext before make any operation.
        let ctx = unsafe {
            &mut *(kernel::container_of!(work_struct, SwornDiskContext, rw_worker)
                as *mut SwornDiskContext)
        } as &mut SwornDiskContext;
        let _lock = ctx.lock.lock();

        let aead = &ctx.aead;
        let checkpoint = &mut ctx.checkpoint;
        let client = &mut ctx.dm_io_client;
        let data_seg_buffer = &mut ctx.data_seg_buffer;
        let index_seg = &mut ctx.index_seg;
        let memtable = &mut ctx.memtable;
        let data_dev = &mut ctx.data_dev;
        let meta_dev = &mut ctx.meta_dev;
        let data_bdev = &data_dev.block_device()?;
        let meta_bdev = &meta_dev.block_device()?;
        let indirect_block_cache = &mut ctx.indirect_block_cache;
        let leaf_block_cache = &mut ctx.leaf_block_cache;

        while let Some(mut bio) = ctx.bio_queue.pop_front() {
            let sector = bio.sector();
            let operation = bio.operation();
            let res = match operation {
                READ => Self::handle_read_request(
                    &mut bio,
                    aead,
                    checkpoint,
                    client,
                    data_seg_buffer,
                    memtable,
                    data_bdev,
                    meta_bdev,
                    indirect_block_cache,
                    leaf_block_cache,
                ),

                WRITE => Self::handle_write_request(
                    &mut bio,
                    aead,
                    checkpoint,
                    client,
                    data_seg_buffer,
                    index_seg,
                    memtable,
                    data_bdev,
                    meta_bdev,
                ),

                _ => {
                    unsafe { bio.end() };
                    Ok(())
                }
            };

            // TODO: there should be a handler for EBADMSG error instead of discard the Bio @kirainmoe
            match res {
                Ok(()) => {}
                Err(e) => {
                    pr_info!(
                        "error {:?} while processing bio: operation = {}, sector = {}",
                        e,
                        operation,
                        sector
                    );
                    unsafe { bio.end() };
                }
            }
        }

        Ok(())
    }
}

impl RwWorker {
    fn handle_read_request(
        bio: &mut Bio,
        aead: &Pin<Box<Aead>>,
        checkpoint: &mut Checkpoint,
        client: &mut DmIoClient,
        data_seg_buffer: &mut DataSegment,
        memtable: &mut MemTable,
        data_bdev: &BlockDevice,
        meta_bdev: &BlockDevice,
        indirect_block_cache: &mut LruCache<u64, IndirectBlock>,
        leaf_block_cache: &mut LruCache<u64, LeafBlock>,
    ) -> Result {
        let begin_sector = bio.sector();
        let len = bio.size();
        let (begin_lba, end_lba, begin_offset, end_offset) =
            get_lba_range(begin_sector, len as u64);

        let block_size = BLOCK_SIZE as usize;

        let mut buf_offset = 0;
        let mut buf = Vec::new();
        buf.try_resize(len as usize, 0u8)?;

        'traverse_lba: for lba in begin_lba..end_lba {
            let len = if lba == begin_lba {
                core::cmp::min(block_size - begin_offset, len as usize)
            } else if lba == end_lba - 1 {
                end_offset
            } else {
                block_size
            };

            // begin offset of current LBA
            let offset = if lba == begin_lba { begin_offset } else { 0 };

            // find in data segment buffer
            if let Some(len) = data_seg_buffer.read(
                lba as u64,
                &mut buf[buf_offset..buf_offset + len],
                offset,
                len,
            ) {
                buf_offset += len;
                continue;
            }

            // find in memtable
            if let Some(record) = memtable.find(lba as u64) {
                let mut block = Self::read_block_with_record(&record, data_bdev, client)?;
                Self::decrypt_block(&mut block, &record, aead)?;

                buf[buf_offset..buf_offset + len].copy_from_slice(&block[offset..offset + len]);
                buf_offset += len;

                continue;
            }

            // find in LSM-tree (BIT)
            for level in 1..LSM_TREE_MAX_LEVEL {
                for root_meta in checkpoint.bit_category.iter_level(level)? {
                    if !root_meta.contains(lba as u64) {
                        continue;
                    }

                    let bit =
                        root_meta.read_from_disk(aead, meta_bdev, client, indirect_block_cache)?;

                    let record = bit.find_record(
                        lba as u64,
                        aead,
                        meta_bdev,
                        client,
                        indirect_block_cache,
                        leaf_block_cache,
                    )?;

                    if record.is_some() {
                        let record = record.unwrap();
                        let mut block = Self::read_block_with_record(&record, data_bdev, client)?;
                        Self::decrypt_block(&mut block, &record, aead)?;

                        buf[buf_offset..buf_offset + len]
                            .copy_from_slice(&block[offset..offset + len]);
                        buf_offset += len;

                        continue 'traverse_lba;
                    }
                }
            }
        }

        bio.set_data(buf)?;
        unsafe { bio.end() };

        Ok(())
    }

    fn handle_write_request(
        bio: &mut Bio,
        aead: &Pin<Box<Aead>>,
        checkpoint: &mut Checkpoint,
        client: &mut DmIoClient,
        data_seg_buffer: &mut DataSegment,
        index_seg: &mut IndexSegment,
        memtable: &mut MemTable,
        bdev: &BlockDevice,
        meta_bdev: &BlockDevice,
    ) -> Result {
        let begin_sector = bio.sector();
        let (data, len) = bio.data(0)?;

        // get the LBA range of current write request: [begin_lba, end_lba)
        let (begin_lba, end_lba, begin_offset, end_offset) =
            get_lba_range(begin_sector, len as u64);

        let mut buf_offset = 0;
        let block_size = BLOCK_SIZE as usize;

        for lba in begin_lba..end_lba {
            // write length of current LBA
            let len = if lba == begin_lba {
                core::cmp::min(block_size - begin_offset, len)
            } else if lba == end_lba - 1 {
                end_offset
            } else {
                block_size
            };

            // begin offset of current LBA
            let offset = if lba == begin_lba { begin_offset } else { 0 };

            // the slice range of data
            let block = &data[buf_offset..buf_offset + len];
            buf_offset += len;

            // log the block to data segment
            data_seg_buffer.write(
                lba as u64, block, offset, len, aead, checkpoint, client, memtable, bdev,
            )?;

            // if the memtable reaches the threshold, trigger writeback
            if memtable.size() >= MEMTABLE_THRESHOLD {
                let bit =
                    BIT::from_memtable(memtable, aead, client, checkpoint, meta_bdev, index_seg)?;
                checkpoint.bit_category.add_bit(bit)?;
                memtable.clear();
            }
        }

        // SAFETY: Safe, we owns the bio in a write request.
        unsafe { bio.end() };

        Ok(())
    }

    fn read_block_with_record(
        record: &Record,
        bdev: &BlockDevice,
        client: &mut DmIoClient,
    ) -> Result<Vec<u8>> {
        let mut block = Vec::new();
        block.try_resize(BLOCK_SIZE as usize, 0u8)?;

        let mut region = DmIoRegion::new(&bdev, record.hba, BLOCK_SECTORS)?;
        let mut io_req = DmIoRequest::with_kernel_memory(
            READ as i32, // req_op
            READ as i32, // req_op_flags
            block.as_mut_ptr() as *mut c_void,
            0, // offset
            client,
        );
        io_req.submit(&mut region);

        Ok(block)
    }

    fn decrypt_block(block: &mut Vec<u8>, record: &Record, aead: &Pin<Box<Aead>>) -> Result {
        unsafe {
            aead.as_ref().decrypt_in_place(
                &slice_to_vec::<{ SWORNDISK_KEY_LENGTH }>(&record.key)?,
                &mut slice_to_vec::<{ SWORNDISK_MAC_LENGTH }>(&record.mac)?,
                &mut slice_to_vec::<{ SWORNDISK_NONCE_LENGTH }>(&record.nonce)?,
                block,
                BLOCK_SIZE as usize,
            )
        }
    }
}
