use crate::{
    context::{SwornDiskContext, CONTEXT as context},
    prelude::*,
    regions::{Record, BIT},
    utils::{get_lba_range, slice_to_vec},
};

pub struct IoWorker;

impl WorkFuncTrait for IoWorker {
    /// functions to handle block I/O request asynchronously
    fn work(_work_struct: *mut bindings::work_struct) -> Result {
        // SAFETY: Safe. `context.lock` is acquired when accessing the member of `context`.
        let ctx = unsafe { context.as_mut().unwrap() };

        loop {
            let bio = {
                let bio_queue_lock = ctx.bio_queue.as_mut();
                let mut bio_queue = bio_queue_lock.lock();
                bio_queue.pop_front()
            };

            if let Some(mut bio) = bio {
                let sector = bio.sector();
                let operation = bio.operation();
                let res = match operation {
                    READ => Self::handle_read_request(&mut bio, ctx),
                    WRITE => Self::handle_write_request(&mut bio, ctx),
                    _ => {
                        // SAFETY: Safe. we owns the bio.
                        unsafe { bio.end() };
                        Ok(())
                    }
                };

                match res {
                    Ok(()) => {}
                    Err(e) => {
                        pr_info!(
                            "error {:?} while processing bio: operation = {}, sector = {}",
                            e,
                            operation,
                            sector
                        );

                        // SAFETY: Safe. we owns the bio.
                        unsafe { bio.end() };
                    }
                }
            } else {
                break;
            }
        }

        Ok(())
    }
}

impl IoWorker {
    fn handle_read_request(bio: &mut Bio, ctx: &mut SwornDiskContext) -> Result {
        let _lock = ctx.lock.read();

        let aead = &ctx.aead;
        let checkpoint = &mut ctx.checkpoint;
        let client = &ctx.dm_io_client;
        let data_seg_buffer = &mut ctx.data_seg_buffer;
        let memtable = &mut ctx.memtable;
        let data_dev = &mut ctx.data_dev;
        let meta_dev = &mut ctx.meta_dev;
        let data_bdev = &data_dev.block_device()?;
        let meta_bdev = &meta_dev.block_device()?;
        let indirect_block_cache = &mut ctx.indirect_block_cache;
        let leaf_block_cache = &mut ctx.leaf_block_cache;

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
            for level in 0..LSM_TREE_MAX_LEVEL {
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

    fn handle_write_request(bio: &mut Bio, ctx: &mut SwornDiskContext) -> Result {
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

            let _lock = ctx.lock.write();

            let aead = &ctx.aead;
            let checkpoint = &mut ctx.checkpoint;
            let client = &ctx.dm_io_client;
            let data_seg_buffer = &mut ctx.data_seg_buffer;
            let index_seg = &mut ctx.index_seg;
            let memtable = &mut ctx.memtable;
            let data_dev = &mut ctx.data_dev;
            let meta_dev = &mut ctx.meta_dev;
            let data_bdev = &data_dev.block_device()?;
            let meta_bdev = &meta_dev.block_device()?;

            // log the block to data segment
            data_seg_buffer.write(
                lba as u64, block, offset, len, aead, checkpoint, client, memtable, data_bdev,
            )?;

            // if the memtable reaches the threshold, trigger writeback (minor compaction)
            if memtable.size() >= MEMTABLE_THRESHOLD {
                pr_info!("Memtable size: {}", memtable.size());

                let bit =
                    BIT::from_memtable(memtable, aead, client, checkpoint, meta_bdev, index_seg)?;
                checkpoint.bit_category.add_bit(bit, 0)?;
                memtable.clear();

                if checkpoint.bit_category.is_compaction_required() {
                    ctx.work_queue.queue_work(&mut ctx.compaction_worker);
                }
            }
        }

        // SAFETY: Safe, we owns the bio in a write request.
        unsafe { bio.end() };

        Ok(())
    }

    fn read_block_with_record(
        record: &Record,
        bdev: &BlockDevice,
        client: &DmIoClient,
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
