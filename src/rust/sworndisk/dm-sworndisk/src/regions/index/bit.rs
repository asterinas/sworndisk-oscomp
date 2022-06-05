use super::{memtable::MemTable, record::Record, segment::IndexSegment};
use crate::{prelude::*, regions::Checkpoint, utils::*};

/// # SwornDisk Linux Rust: BIT Implementation Design
///
///               RootRecord(IndirectRecord)
///               manages  |
///                        v
///               RootBlock(IndirectBlock)
///                  /               \
///            IndirectRecord       IndirectRecord
///           manages |          manages |
///                   v                  v
///            IndirectBlock         IndirectBlock
///              /           \
///          IndirectRecord IndirectRecord
///               | manages
///               v
///            LeafBlock
///            /         \
///          Record     Record

#[derive(Copy, Clone, Debug)]
/// Leaf node of BIT
pub struct LeafRecord {
    /// logical block address of current record
    pub lba: u64,
    /// hba & key & nonce & mac
    pub record: Record,
}

#[derive(Debug)]
/// On-disk unit of leaf node
pub struct LeafBlock {
    /// number of LeafRecord
    pub count: usize,
    /// children vector
    pub children: Vec<LeafRecord>,
}

#[derive(Copy, Clone, Debug)]
/// Indirect node of BIT
pub struct IndirectRecord {
    /// lba range of IndirectBlock responding to this IndirectRecord
    pub lba_range: (u64, u64),
    /// hba & key & nonce & mac
    pub record: Record,
}

#[derive(Debug)]
/// On-disk unit of indirect node
pub struct IndirectBlock {
    /// number of IndirectRecord
    pub count: usize,
    /// children vector
    pub children: Vec<IndirectRecord>,
}

/// Size of struct LeafRecord
pub const LEAF_RECORD_SIZE: usize = mem::size_of::<LeafRecord>();
/// Size of struct IndirectRecord
pub const INDIRECT_RECORD_SIZE: usize = mem::size_of::<IndirectRecord>();
/// Size of struct LeafBlock
pub const LEAF_BLOCK_CHILDREN: usize = (BLOCK_SIZE as usize - 8) / LEAF_RECORD_SIZE; // 8: count(u64)
/// Size of struct IndirectBlock
pub const INDIRECT_BLOCK_CHILDREN: usize = (BLOCK_SIZE as usize - 8) / INDIRECT_RECORD_SIZE; // 8: count(u64)

impl Default for LeafRecord {
    fn default() -> Self {
        Self {
            lba: u64::MAX,
            record: Record::default(),
        }
    }
}

impl Default for IndirectRecord {
    fn default() -> Self {
        Self {
            lba_range: (u64::MAX, u64::MAX),
            record: Record::default(),
        }
    }
}

impl Default for LeafBlock {
    fn default() -> Self {
        Self {
            count: 0,
            children: Vec::new(),
        }
    }
}

impl Default for IndirectBlock {
    fn default() -> Self {
        Self {
            count: 0,
            children: Vec::new(),
        }
    }
}

impl Clone for LeafBlock {
    fn clone(&self) -> Self {
        let mut children = Vec::new();
        children.try_extend_from_slice(&self.children).unwrap();
        Self {
            children,
            count: self.count,
        }
    }
}

impl Clone for IndirectBlock {
    fn clone(&self) -> Self {
        let mut children = Vec::new();
        children.try_extend_from_slice(&self.children).unwrap();
        Self {
            children,
            count: self.count,
        }
    }
}

impl Serialize for LeafRecord {
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();
        vec.try_extend_from_slice(&unsafe { mem::transmute::<u64, [u8; 8]>(self.lba) })?;
        vec.try_extend_from_slice(&self.record.serialize()?)?;

        assert_eq!(vec.len() <= LEAF_RECORD_SIZE, true);
        vec.try_resize(LEAF_RECORD_SIZE, 0u8)?;
        Ok(vec)
    }
}

impl Deserialize for LeafRecord {
    fn deserialize(buffer: &[u8]) -> Result<Self> {
        if buffer.len() != LEAF_RECORD_SIZE {
            return Err(EINVAL);
        }

        let lba = unsafe { mem::transmute::<[u8; 8], u64>(buffer[0..8].try_into().unwrap()) };
        let record = Record::deserialize(&buffer[8..8 + SWORNDISK_RECORD_SIZE])?;

        Ok(LeafRecord { lba, record })
    }
}

impl Serialize for LeafBlock {
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();
        vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(self.count) })?;
        for item in self.children.iter() {
            vec.try_extend_from_slice(&item.serialize()?)?;
        }

        assert_eq!(vec.len() <= BLOCK_SIZE as usize, true);
        vec.try_resize(BLOCK_SIZE as usize, 0u8)?;
        Ok(vec)
    }
}

impl Deserialize for LeafBlock {
    fn deserialize(buffer: &[u8]) -> Result<Self> {
        if buffer.len() != BLOCK_SIZE as usize {
            return Err(EINVAL);
        }

        let count = unsafe { mem::transmute::<[u8; 8], usize>(buffer[0..8].try_into().unwrap()) };
        let mut children = Vec::new();
        for i in 0..LEAF_BLOCK_CHILDREN {
            let index = 8 + i * LEAF_RECORD_SIZE;
            children.try_push(LeafRecord::deserialize(
                &buffer[index..index + LEAF_RECORD_SIZE],
            )?)?;
        }

        Ok(Self { count, children })
    }
}

impl Serialize for IndirectRecord {
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();
        vec.try_extend_from_slice(&unsafe {
            mem::transmute::<(u64, u64), [u8; 16]>(self.lba_range)
        })?;
        vec.try_extend_from_slice(&self.record.serialize()?)?;

        assert_eq!(vec.len() <= INDIRECT_RECORD_SIZE, true);
        vec.try_resize(INDIRECT_RECORD_SIZE, 0u8)?;
        Ok(vec)
    }
}

impl Deserialize for IndirectRecord {
    fn deserialize(buffer: &[u8]) -> Result<Self> {
        if buffer.len() != INDIRECT_RECORD_SIZE {
            return Err(EINVAL);
        }
        let lba_range =
            unsafe { mem::transmute::<[u8; 16], (u64, u64)>(buffer[0..16].try_into().unwrap()) };
        let record = Record::deserialize(&buffer[16..16 + SWORNDISK_RECORD_SIZE])?;
        Ok(Self { lba_range, record })
    }
}

impl Serialize for IndirectBlock {
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();
        vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(self.count) })?;
        for item in self.children.iter() {
            vec.try_extend_from_slice(&item.serialize()?)?;
        }

        assert_eq!(vec.len() <= BLOCK_SIZE as usize, true);
        vec.try_resize(BLOCK_SIZE as usize, 0u8)?;
        Ok(vec)
    }
}

impl Deserialize for IndirectBlock {
    fn deserialize(buffer: &[u8]) -> Result<Self> {
        if buffer.len() != BLOCK_SIZE as usize {
            return Err(EINVAL);
        }
        let count = unsafe { mem::transmute::<[u8; 8], usize>(buffer[0..8].try_into().unwrap()) };
        let mut children = Vec::new();
        for i in 0..INDIRECT_BLOCK_CHILDREN {
            let index = 8 + i * INDIRECT_RECORD_SIZE;
            children.try_push(IndirectRecord::deserialize(
                &buffer[index..index + INDIRECT_RECORD_SIZE],
            )?)?;
        }
        Ok(Self { count, children })
    }
}

impl LeafBlock {
    pub fn get_lba_range(&self) -> (u64, u64) {
        match self.count {
            0 => (u64::MAX, u64::MIN),
            _ => (self.children[0].lba, self.children[self.count - 1].lba),
        }
    }

    pub fn is_full(&self) -> bool {
        self.count == LEAF_BLOCK_CHILDREN
    }

    pub fn push(&mut self, record: LeafRecord) {
        self.children.try_push(record).unwrap();
        self.count += 1;

        assert_eq!(self.count <= LEAF_BLOCK_CHILDREN, true);
    }
}

impl IndirectBlock {
    pub fn get_lba_range(&self) -> (u64, u64) {
        match self.count {
            0 => (u64::MAX, u64::MIN),
            _ => (
                self.children[0].lba_range.0,
                self.children[self.count - 1].lba_range.1,
            ),
        }
    }

    pub fn is_full(&self) -> bool {
        self.count == INDIRECT_BLOCK_CHILDREN
    }

    pub fn push(&mut self, record: IndirectRecord) {
        self.children.try_push(record).unwrap();
        self.count += 1;

        assert_eq!(self.count <= INDIRECT_BLOCK_CHILDREN, true);
    }
}

#[derive(Debug)]
pub struct BIT {
    /// root node of BIT
    pub root: IndirectBlock,

    /// IndirectRecord of root node
    pub record: IndirectRecord,

    /// max level of BIT
    pub level: usize,
}

impl BIT {
    /// Create BIT from a MemTable
    pub fn from_memtable(
        memtable: &MemTable,
        aead: &Pin<Box<Aead>>,
        client: &mut DmIoClient,
        checkpoint: &mut Checkpoint,
        meta_bdev: &BlockDevice,
        index_seg: &mut IndexSegment,
    ) -> Result<Self> {
        // calcualte the max level of BIT, the max level should not exceeded the BIT_MAX_LEVEL
        let mut max_level = 2; // minimum level is 2 (IndirectBlock + LeafBlock)
        let mut size = memtable.size() / LEAF_BLOCK_CHILDREN / INDIRECT_BLOCK_CHILDREN;
        while size > 0 {
            size /= INDIRECT_BLOCK_CHILDREN;
            max_level += 1;
        }
        if max_level >= BIT_MAX_LEVEL {
            return Err(ENOSPC);
        }

        // array stores current level's LeafBlock / IndirectBlock node
        // this will take a space of BIT_MAX_LEVEL pages
        let mut leaf = Box::try_new(LeafBlock::default())?;
        let mut indirect = Box::try_new([(); BIT_MAX_LEVEL].map(|_| IndirectBlock::default()))?;

        // move elements from MemTable to BIT
        let mut index = 0;
        let size = memtable.size();

        pr_info!("MemTable total size: {}", size);

        for (lba, record) in memtable.iter() {
            leaf.push(LeafRecord {
                lba: *lba,
                record: *record,
            });

            index += 1;
            // when LeafBlock is full or is the last element, trigger a writeback
            if index == size || leaf.is_full() {
                let lba_range = leaf.get_lba_range();
                let record = Self::writeback_block(
                    leaf.as_ref(),
                    aead,
                    client,
                    checkpoint,
                    meta_bdev,
                    index_seg,
                )?;

                indirect[max_level - 2].push(IndirectRecord { lba_range, record });

                Self::pushup_indirect_block(
                    &mut indirect,
                    max_level,
                    false, // index < size
                    aead,
                    client,
                    checkpoint,
                    meta_bdev,
                    index_seg,
                )?;

                *leaf = LeafBlock::default();
            }
        }

        let root_record = Self::pushup_indirect_block(
            &mut indirect,
            max_level,
            true,
            aead,
            client,
            checkpoint,
            meta_bdev,
            index_seg,
        )?
        .unwrap();

        let mut root_block = IndirectBlock::default();
        mem::swap(&mut root_block, &mut indirect[0]);

        Ok(Self {
            root: root_block,
            record: root_record,
            level: max_level,
        })
    }

    pub fn find_record(
        &self,
        lba: u64,
        aead: &Pin<Box<Aead>>,
        bdev: &BlockDevice,
        client: &mut DmIoClient,
        indirect_block_cache: &mut LruCache<u64, IndirectBlock>,
        leaf_block_cache: &mut LruCache<u64, LeafBlock>,
    ) -> Result<Option<Record>> {
        // If the block does not contains the LBA, return not found
        let lba_range = self.root.get_lba_range();
        if lba < lba_range.0 || lba > lba_range.1 {
            return Ok(None);
        }

        let mut block: Option<IndirectBlock> = None;

        // First we traverse down the IndirectBlock, find the LeafBlock
        // The level of IndirectBlock (except root) is [1..level - 2)
        'traverse_each_level: for level in 0..self.level - 2 {
            let children = if level == 0 {
                &self.root.children
            } else {
                block.as_ref().map(|m| &m.children).unwrap()
            };

            let count = if level == 0 {
                self.root.count
            } else {
                block.as_ref().map(|m| m.count).unwrap()
            };

            // search children until find the target IndirectRecord
            match Self::find_indirect_record(children, count, lba) {
                Some(indirect_record) => {
                    drop(children);
                    drop(count);

                    let record = indirect_record.record;
                    block = Some(Self::read_block(
                        record,
                        aead,
                        bdev,
                        client,
                        indirect_block_cache,
                    )?);
                    continue 'traverse_each_level;
                }
                None => {}
            };

            // if arrives here, that we do not find a block contains the LBA
            return Ok(None);
        }

        // Now we gonna read the leafblock and find the record
        let children = if self.level == 2 {
            &self.root.children
        } else {
            block.as_ref().map(|m| &m.children).unwrap()
        };

        let count = if self.level == 2 {
            self.root.count
        } else {
            block.as_ref().map(|m| m.count).unwrap()
        };

        match Self::find_indirect_record(children, count, lba) {
            Some(indirect_record) => {
                let record = indirect_record.record;
                let leaf: LeafBlock =
                    Self::read_block(record, aead, bdev, client, leaf_block_cache)?;
                return Ok(Self::find_record_in_leafblock(&leaf, lba));
            }
            None => Ok(None),
        }
    }

    /// binary search a Record with LBA in LeafBlock
    fn find_record_in_leafblock(leafblock: &LeafBlock, lba: u64) -> Option<Record> {
        let mut left = 0;
        let mut right = leafblock.count - 1;
        while left <= right {
            let mid = (left + right) >> 1;
            let mid_lba = leafblock.children[mid].lba;
            if lba == mid_lba {
                return Some(leafblock.children[mid].record);
            } else if mid == 0 {
                return None; // fix: usize subtract overflow
            } else if lba < mid_lba {
                right = mid - 1;
            } else {
                left = mid + 1;
            }
        }
        return None;
    }

    /// binary search a IndirectRecord that may contains the LBA
    fn find_indirect_record(
        children: &Vec<IndirectRecord>,
        count: usize,
        lba: u64,
    ) -> Option<IndirectRecord> {
        let mut left = 0;
        let mut right = count - 1;
        while left <= right {
            let mid = (left + right) >> 1;
            let lba_range = children[mid].lba_range;
            if lba_range.0 <= lba && lba <= lba_range.1 {
                return Some(children[mid]);
            } else if mid == 0 {
                return None; // fix: usize subtract overflow
            } else if lba < lba_range.0 {
                right = mid - 1;
            } else {
                left = mid + 1;
            }
        }
        return None;
    }

    /// Consecutively writeback IndirectBlock to upper level.
    ///
    /// If `write_all` is specified, all nodes will be write to disk immediately no matter they are full or not.
    fn pushup_indirect_block(
        indirect: &mut [IndirectBlock; BIT_MAX_LEVEL],
        max_level: usize,
        write_all: bool,
        aead: &Pin<Box<Aead>>,
        client: &mut DmIoClient,
        checkpoint: &mut Checkpoint,
        meta_bdev: &BlockDevice,
        index_seg: &mut IndexSegment,
    ) -> Result<Option<IndirectRecord>> {
        let mut level = max_level - 2;
        loop {
            if write_all || indirect[level].is_full() {
                if level > 0 && indirect[level].count == 0 {
                    level -= 1;
                    continue;
                }

                let lba_range = indirect[level].get_lba_range();
                let record = Self::writeback_block(
                    &indirect[level],
                    aead,
                    client,
                    checkpoint,
                    meta_bdev,
                    index_seg,
                )?;
                if level > 0 {
                    indirect[level - 1].push(IndirectRecord { lba_range, record });
                    indirect[level] = IndirectBlock::default();
                    level -= 1;
                } else {
                    return Ok(Some(IndirectRecord { lba_range, record }));
                }
            } else {
                break;
            }
        }

        return Ok(None);
    }

    /// Read a struct of size BLOCK_SIZE. The struct should be deserializable.  
    fn read_block<T: Sized + Deserialize + Debug + Clone>(
        record: Record,
        aead: &Pin<Box<Aead>>,
        bdev: &BlockDevice,
        client: &mut DmIoClient,
        cache: &mut LruCache<u64, T>,
    ) -> Result<T> {
        match cache.get(&record.hba) {
            Some(block) => {
                return Ok(block.clone());
            }
            _ => {}
        };

        let mut block = Vec::new();
        block.try_resize(BLOCK_SIZE as usize, 0u8)?;

        let mut region = DmIoRegion::new(&bdev, record.hba, BLOCK_SECTORS)?;
        let mut io_req = DmIoRequest::with_kernel_memory(
            READ as i32,
            READ as i32,
            block.as_mut_ptr() as *mut c_void,
            0,
            client,
        );
        io_req.submit(&mut region);

        unsafe {
            aead.as_ref().decrypt_in_place(
                &slice_to_vec::<{ SWORNDISK_KEY_LENGTH }>(&record.key)?,
                &mut slice_to_vec::<{ SWORNDISK_MAC_LENGTH }>(&record.mac)?,
                &mut slice_to_vec::<{ SWORNDISK_NONCE_LENGTH }>(&record.nonce)?,
                &mut block,
                BLOCK_SIZE as usize,
            )?
        };

        let result = T::deserialize(&block)?;

        // insert and update LRU cache
        cache.put(record.hba, result.clone())?;

        Ok(result)
    }

    /// Write a Block which implemented Serialize trait to disk
    fn writeback_block<T: Sized + Serialize>(
        block: &T,
        aead: &Pin<Box<Aead>>,
        client: &mut DmIoClient,
        checkpoint: &mut Checkpoint,
        meta_bdev: &BlockDevice,
        index_seg: &mut IndexSegment,
    ) -> Result<Record> {
        let buf = block.serialize()?;
        let record = index_seg.write(
            &buf,
            BLOCK_SIZE as usize,
            aead,
            client,
            checkpoint,
            meta_bdev,
        )?;

        Ok(record)
    }
}
