use crate::{context::CONTEXT as context, prelude::*, regions::BIT};

/// SwornDisk Compaction implementation
pub struct CompactionWorker;

impl WorkFuncTrait for CompactionWorker {
    fn work(_work_struct: *mut bindings::work_struct) -> Result {
        let ctx = unsafe { context.as_mut().unwrap() };

        let aead = &ctx.aead;
        let client = &ctx.dm_io_client;
        let checkpoint = &mut ctx.checkpoint;
        let index_seg = &mut ctx.index_seg;
        let meta_dev = &mut ctx.meta_dev;
        let meta_bdev = &meta_dev.block_device()?;
        let indirect_block_cache = &mut ctx.indirect_block_cache;

        let mut bits_pending_compaction = Vec::new();
        let mut bits_id = Vec::new();

        pr_info!("Triggered major compaction...");

        for level in 0..LSM_TREE_MAX_LEVEL - 1 {
            // TODO: refactor lock, write lock should only be acquired when write the BIT
            let _lock = ctx.lock.write();

            let size = checkpoint.bit_category.level_size(level);
            if size >= MAX_COMPACTION_NUMBER {
                for i in 0..size {
                    let root_meta = checkpoint.bit_category.get_bit(level, i).unwrap();
                    let root =
                        root_meta.read_from_disk(aead, meta_bdev, client, indirect_block_cache)?;
                    bits_pending_compaction.try_push(root)?;
                    bits_id.try_push(root_meta.unique_id)?;
                }
            }

            if bits_pending_compaction.len() <= 0 {
                continue;
            }

            let bit = BIT::from_compaction(
                &bits_pending_compaction,
                aead,
                client,
                checkpoint,
                meta_bdev,
                index_seg,
            )?;

            pr_info!("new BIT created: {:?}", bit);

            // add BIT meta info to BITCategory
            checkpoint.bit_category.add_bit(bit, level + 1)?;

            // remove old, compacted BITs
            for idx in bits_id.iter() {
                checkpoint.bit_category.release_bit(level, *idx)?;
            }

            // clear pending compaction queue
            bits_id.clear();
            bits_pending_compaction.clear();
        }

        Ok(())
    }
}
