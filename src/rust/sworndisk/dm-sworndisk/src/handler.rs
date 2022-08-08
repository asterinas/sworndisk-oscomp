//! Device Mapper target event handlers of SwornDisk

use crate::{
    context::{SwornDiskContext, CONTEXT as context},
    prelude::*,
    regions::{Checkpoint, DataSegment, IndexSegment, MemTable, SuperBlock},
    utils::{current_timestamp, DebugIgnore, LruCache},
};

use kernel::sync::{Mutex, RwSemaphore};

pub(crate) struct DmSwornDiskHandler;

impl DmCallbacks for DmSwornDiskHandler {
    // declares that this device mapper handler defined `dm_ctr_fn`, `dm_dtr_fn` and `dm_map_fn`
    declare_device_mapper_callbacks!(ctr, dtr, map);

    /// Constructor of SwornDisk device mapper target
    ///
    /// # Usage
    ///
    /// `dmsetup create <target_name> <start> <nr_sector> sworndisk <data_dev_path> <meta_dev_path> <start_sector> <should_format>`
    ///
    /// # Exaple
    ///
    /// `dmsetup create test-sworndisk 0 58593750 sworndisk /dev/loop0 0` will create a
    /// SwornDisk device mapper at `/dev/mapper/test-sworndisk` of size 30GiB (58593750 sectors).
    fn ctr(mut target: DmTarget, args: Vec<&'static CStr>) -> Result<i32> {
        // Check argument length should contain [dev_path, start_sector, force_format]
        let argc = args.len();
        if argc != 4 {
            pr_warn!("Invalid arguments to construct a SwornDisk.");
            pr_warn!("Accept paramteters: <data_dev> <meta_dev> <start_sector> <should_format>");
            return Err(EINVAL);
        }

        // Set device mapper device
        let mut data_dev = DmDev::new()?;
        let path = args[0];
        target.get_device(path, 0, &mut data_dev);

        let mut meta_dev = DmDev::new()?;
        let path = args[1];
        target.get_device(path, 0, &mut meta_dev);

        // Create device mapper block manager
        let block_manager = DmBlockManager::new(
            meta_dev.block_device()?,
            BLOCK_SIZE as u32,
            MAX_CONCURRENT_LOCKS,
        )?;

        // Read superblock from disk. If there is no valid superblock, format a SwornDisk.
        // Calulate the size of each segment.
        let format_type = str::from_utf8(args[3].as_bytes())?;
        let data_nbytes = data_dev.block_device()?.bd_nr_sectors() as u64 * SECTOR_SIZE;
        let meta_nbytes = meta_dev.block_device()?.bd_nr_sectors() as u64 * SECTOR_SIZE;
        let index_nbytes = meta_nbytes / 2;
        let journal_nbytes = meta_nbytes / 4;

        let (superblock, should_init) = match format_type {
            // if `format_type` is "force", then we are going to create an empty SwornDisk.
            "force" => {
                pr_warn!("`FORCE_FORMAT` is enabled, will create an empty SwornDisk.");
                (
                    Self::new_superblock(
                        data_nbytes,
                        index_nbytes,
                        journal_nbytes,
                        &block_manager,
                    )?,
                    true,
                )
            }
            _ => Self::read_superblock(
                data_nbytes,
                index_nbytes,
                journal_nbytes,
                &block_manager,
                format_type,
            )?,
        };

        pr_info!("SuperBlock: {:?}", superblock);

        // Create a device mapper I/O client
        let mut dm_io_client = DmIoClient::new();

        // Read or create checkpoint
        let mut checkpoint = match should_init {
            true => Checkpoint::new(
                superblock.data_segments_number(),
                superblock.index_segments_number(),
            )?,
            false => {
                pr_info!("Reading existed Checkpoint.");

                let checkpoint_hba = superblock.checkpoint_region / SECTOR_SIZE;
                let checkpoint_ondisk = Checkpoint::read_from_disk(
                    &meta_dev.block_device()?,
                    &mut dm_io_client,
                    checkpoint_hba,
                )?;
                checkpoint_ondisk
            }
        };

        // Create a data segment buffer
        // TODO: Multi logging head
        // TODO: Handle the situation if there is no segment left
        let data_seg_buffer = {
            let data_seg_index = checkpoint.data_svt.alloc()?;
            let hba = (data_seg_index as u64) * SEGMENT_SECTORS;
            DataSegment::new(hba)?
        };

        let index_seg = IndexSegment::new(SEGMENT_SECTORS);

        // Create an in-memory index tree
        let memtable = MemTable::new();

        // Create a work queue to handle async works
        let work_queue = WorkQueue::new(
            c_str!("queue"),
            bindings::WQ_UNBOUND | bindings::WQ_MEM_RECLAIM,
            0,
        )?;

        // Create a queue to handle async bio requests
        // SAFETY: `kernel::mutex_init!()` is called below.
        let mut bio_queue = Pin::from(Box::try_new(unsafe { Mutex::new(LinkedList::new()) })?);
        kernel::mutex_init!(bio_queue.as_mut(), "SwornDiskContext::bio_queue");

        // SAFETY: `kernel::rwsemaphre_init!()` is called below.
        let mut lock = Pin::from(Box::try_new(unsafe { RwSemaphore::new(()) })?);
        kernel::rwsemaphore_init!(lock.as_mut(), "SwornDiskContext::lock");

        let indirect_block_cache = LruCache::new(LRU_CACHE_MAX_SIZE)?;
        let leaf_block_cache = LruCache::new(LRU_CACHE_MAX_SIZE)?;

        // Create a global context
        let sworndisk_context = SwornDiskContext {
            block_manager,
            checkpoint,
            data_seg_buffer,
            dm_io_client,
            data_dev,
            index_seg,
            indirect_block_cache,
            leaf_block_cache,
            meta_dev,
            memtable,
            superblock,
            work_queue,

            aead: Aead::new(c_str!("gcm(aes)"), 0, 0)?,
            bio_queue: DebugIgnore(bio_queue),
            lock: DebugIgnore(lock),
            start: str::from_utf8(args[2].as_bytes())?
                .parse::<u64>()
                .map_err(|_| EINVAL)?,
            rw_worker: [(); 6].map(|_| WorkStruct::new()),
            compaction_worker: WorkStruct::new(),
        };

        // SAFETY: Safe. The `context` will not be accessed before `ctx`.
        // and each access of context should acquire the lock inside `context`.
        unsafe {
            context = Some(Box::leak(Box::try_new(sworndisk_context)?));
            context.as_mut().map(|ctx| ctx.init_workers());
        };

        Ok(0)
    }

    /// Destructor of SwornDisk device mapper target
    fn dtr(target: DmTarget) -> Result {
        // drop the context and unregister device mapper target
        let ctx = unsafe { context.take() };
        if ctx.is_some() {
            let ctx = ctx.unwrap();
            ctx.flush()?;

            target.put_device(&ctx.data_dev);
            target.put_device(&ctx.meta_dev);

            drop(ctx);
        }

        Ok(())
    }

    fn map(_target: DmTarget, mut bio: Bio) -> Result<i32> {
        let ctx = unsafe { context.as_mut().unwrap() };

        {
            let bdev = ctx.data_dev.block_device()?;
            bio.set_dev(&bdev)?;
        };

        // Only process a block one time
        // if bio.sectors() as u64 > BLOCK_SECTORS {
        //     bio.accept_partial(BLOCK_SECTORS as usize);
        // }

        let status = match bio.operation() {
            READ | WRITE | FLUSH => {
                // push the bio to queue
                let bio_queue_lock = ctx.bio_queue.as_mut();
                let mut bio_queue = bio_queue_lock.lock();
                bio_queue.push_back(bio)?;

                // alloc the worker task to a queue
                let worker_nr = current_timestamp() as usize % MAX_WORKERS;
                ctx.work_queue.queue_work(&mut ctx.rw_worker[worker_nr]);

                // target.access_private_mut(|ctx: &mut SwornDiskContext| -> Result {
                //     let bio_queue_lock = ctx.bio_queue.as_mut();
                //     let mut bio_queue = bio_queue_lock.lock();

                //     // let mut worker = Box::try_new(WorkStruct::new())?;
                //     // worker.init::<RwWorker>();
                //     // let leaked_worker = Box::leak(worker);

                //     bio_queue.push_back(bio)?;
                //     ctx.work_queue.queue_work(&mut ctx.rw_worker);
                //     Ok(())
                // })??;

                bindings::DM_MAPIO_SUBMITTED as i32
            }
            _ => bindings::DM_MAPIO_KILL as i32,
        };

        Ok(status)
    }
}

impl DmSwornDiskHandler {
    /// Create a new super block of SwornDisk
    fn new_superblock(
        data_nbytes: u64,
        index_nbytes: u64,
        journal_nbytes: u64,
        block_manager: &DmBlockManager,
    ) -> Result<SuperBlock> {
        let superblock = SuperBlock::new(data_nbytes, index_nbytes, journal_nbytes);
        superblock.write_to_disk(&block_manager)?;

        Ok(superblock)
    }

    /// Try to read super block from disk.
    fn read_superblock(
        data_nbytes: u64,
        index_nbytes: u64,
        journal_nbytes: u64,
        block_manager: &DmBlockManager,
        format_type: &str,
    ) -> Result<(SuperBlock, bool)> {
        match SuperBlock::read_from_disk(&block_manager) {
            Ok(superblock) => Ok((superblock, false)),
            Err(val) => {
                pr_warn!("SwornDisk failed to read superblock from device.");

                if format_type == "true" {
                    Ok((
                        Self::new_superblock(
                            data_nbytes,
                            index_nbytes,
                            journal_nbytes,
                            block_manager,
                        )?,
                        true,
                    ))
                } else {
                    Err(val)
                }
            }
        }
    }
}
