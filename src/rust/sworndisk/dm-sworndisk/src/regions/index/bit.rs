use super::{memtable::MemTable, record::Record, segment::IndexSegment};
use crate::{prelude::*, regions::Checkpoint, utils::*};
use core::marker::PhantomData;

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
        if self.count > 0 {
            assert_eq!(self.children[self.count - 1].lba <= record.lba, true);
        }

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
        if self.count > 0 {
            assert_eq!(
                self.children[self.count - 1].lba_range.1 <= record.lba_range.0,
                true
            );
            assert_eq!(record.lba_range.0 <= record.lba_range.1, true);
        }

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

    /// element number of BIT
    pub size: usize,
}

impl BIT {
    /// return the size of BIT
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn get_lba_range(&self) -> (u64, u64) {
        self.root.get_lba_range()
    }

    /// Create BIT from a MemTable
    pub fn from_memtable(
        memtable: &MemTable,
        aead: &Pin<Box<Aead>>,
        client: &DmIoClient,
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
            size,
            root: root_block,
            record: root_record,
            level: max_level,
        })
    }

    /// Compact some BITs of level `i` into a new BIT of level `i+1`
    pub fn from_compaction(
        bits: &Vec<BIT>,
        aead: &Pin<Box<Aead>>,
        client: &DmIoClient,
        checkpoint: &mut Checkpoint,
        meta_bdev: &BlockDevice,
        index_seg: &mut IndexSegment,
    ) -> Result<Self> {
        // create iterators for the BITs will be compacted,
        // then compute the max-level and max-size of BIT.
        let mut bit_iterators = Vec::new();
        let mut max_size = 0;
        for bit in bits {
            max_size += bit.size();
            bit_iterators.try_push(bit.iter(aead, meta_bdev, client)?)?;
        }

        let mut max_level = 2;
        let mut tmp = max_size / LEAF_BLOCK_CHILDREN / INDIRECT_BLOCK_CHILDREN;
        while tmp > 0 {
            tmp /= INDIRECT_BLOCK_CHILDREN;
            max_level += 1;
        }

        // create a new block buffer to store the new BIT
        let mut leaf = Box::try_new(LeafBlock::default())?;
        let mut indirect = Box::try_new([(); BIT_MAX_LEVEL].map(|_| IndirectBlock::default()))?;

        // iterate all the BITs and find the latest node
        // create a node array that holds current iterator nodes
        let mut nodes = Vec::new();
        for iterator in bit_iterators.iter_mut() {
            nodes.try_push(iterator.next()?)?;
        }

        let total_index = bits.len();
        let mut duplicates = Vec::new();
        duplicates.try_resize(total_index, 0usize)?;

        let mut real_size = 0;

        // find the node to be compaction
        loop {
            let mut baseline: Option<LeafRecord> = None;
            let mut duplicate_number = 1;
            for i in 0..total_index {
                if nodes[i].is_some() {
                    let node = nodes[i].as_ref().unwrap();
                    if baseline.is_none() || node.lba < baseline.as_ref().unwrap().lba {
                        baseline = nodes[i];
                        duplicates[0] = i;
                        duplicate_number = 1;
                    } else if node.lba == baseline.as_ref().unwrap().lba {
                        baseline = nodes[i];
                        duplicates[duplicate_number] = i;
                        duplicate_number += 1;
                    }
                }
            }

            if baseline.is_some() {
                real_size += 1;
                leaf.push(baseline.unwrap());
            }

            if leaf.is_full() || baseline.is_none() {
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

            if baseline.is_none() {
                break;
            }

            for i in 0..duplicate_number {
                let index = duplicates[i];
                nodes[index] = bit_iterators[index].next()?;
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
            size: real_size,
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
        client: &DmIoClient,
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
        client: &DmIoClient,
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

                // pr_info!("indirect level {} lba_range {:?}", level, lba_range);

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
    pub fn read_block<T: Sized + Deserialize + Debug + Clone>(
        record: Record,
        aead: &Pin<Box<Aead>>,
        bdev: &BlockDevice,
        client: &DmIoClient,
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

    /// Read a block directly from meta device without accessing cache
    pub fn read_block_directly<T: Sized + Deserialize + Debug + Clone>(
        record: Record,
        aead: &Pin<Box<Aead>>,
        bdev: &BlockDevice,
        client: &DmIoClient,
    ) -> Result<T> {
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

        Ok(result)
    }

    pub fn iter<'a>(
        &'a self,
        aead: &'a Pin<Box<Aead>>,
        bdev: &'a BlockDevice,
        client: &'a DmIoClient,
    ) -> Result<BITIterator<'a>> {
        Ok(BITIterator::new(self, aead, bdev, client)?)
    }

    /// Write a Block which implemented Serialize trait to disk
    fn writeback_block<T: Sized + Serialize>(
        block: &T,
        aead: &Pin<Box<Aead>>,
        client: &DmIoClient,
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

pub struct BITIterator<'a> {
    _bit: PhantomData<&'a BIT>,

    aead: &'a Pin<Box<Aead>>,
    bdev: &'a BlockDevice,
    client: &'a DmIoClient,

    level: usize,
    total_index: usize,
    has_next: bool,
    index: [usize; BIT_MAX_LEVEL],
    block_stack: Box<[IndirectBlock; BIT_MAX_LEVEL]>,
    leaf_block: Box<LeafBlock>,
}

impl<'a> BITIterator<'a> {
    /// Create a new BIT iterator
    pub fn new(
        bit: &'a BIT,
        aead: &'a Pin<Box<Aead>>,
        bdev: &'a BlockDevice,
        client: &'a DmIoClient,
    ) -> Result<Self> {
        let level = bit.level;

        // block_stack, leaf_block and index array maintained the state of a iterator.
        // Each time we iterate the iterator to get next element, the iterator first update the state of itself to
        // ensure that block_stack and leaf_block are valid.
        let mut index = [1; BIT_MAX_LEVEL];
        let mut block_stack = Box::try_new([(); BIT_MAX_LEVEL].map(|_| IndirectBlock::default()))?;
        let mut leaf_block = Box::try_new(LeafBlock::default())?;

        index[level - 1] = 0;

        // read the first node of each level
        block_stack[0] = bit.root.clone();
        for i in 1..level - 1 {
            if block_stack[i - 1].count <= 0 {
                pr_warn!("Cannot create BITIterator: block is empty");
                return Err(EINVAL);
            }

            // read the first element
            block_stack[i] = BIT::read_block_directly::<IndirectBlock>(
                block_stack[i - 1].children[0].record,
                aead,
                bdev,
                client,
            )?;

            // pr_info!("block level {} lba_range: {:?}", i, block_stack[i].get_lba_range());
        }

        if block_stack[level - 2].count <= 0 {
            pr_warn!("Cannot create BITIterator: block is empty");
            return Err(EINVAL);
        }

        *leaf_block = BIT::read_block_directly::<LeafBlock>(
            block_stack[level - 2].children[0].record,
            aead,
            bdev,
            client,
        )?;

        Ok(Self {
            level,
            index,
            block_stack,
            leaf_block,

            aead,
            bdev,
            client,

            _bit: PhantomData,
            total_index: 0,
            has_next: true,
        })
    }

    /// Update the block stack
    pub fn update_block_stack(&mut self) -> Result {
        // check LeafBlock has next element
        if self.index[self.level - 1] >= self.leaf_block.count {
            if !self.has_next {
                return Ok(());
            }

            let mut level = self.level - 1;
            let next_leaf_block = BIT::read_block_directly::<LeafBlock>(
                self.block_stack[level - 1].children[self.index[level - 1]].record,
                self.aead,
                self.bdev,
                self.client,
            )?;

            *(self.leaf_block) = next_leaf_block;
            self.index[level] = 0;
            self.index[level - 1] += 1;
            level -= 1;

            // recursively update the block stack and index array
            while level > 0 && self.index[level] >= self.block_stack[level].count {
                if level - 1 == 0 && self.index[0] >= self.block_stack[0].count {
                    self.has_next = false;
                    return Ok(());
                }
                let next_block = BIT::read_block_directly::<IndirectBlock>(
                    self.block_stack[level - 1].children[self.index[level - 1]].record,
                    self.aead,
                    self.bdev,
                    self.client,
                )?;
                self.block_stack[level] = next_block;
                self.index[level] = 0;
                self.index[level - 1] += 1;
                level -= 1;
            }
        }

        Ok(())
    }

    pub fn has_next(&self) -> bool {
        self.has_next || self.index[self.level - 1] < self.leaf_block.count
    }

    /// Iterate next element
    pub fn next(&mut self) -> Result<Option<LeafRecord>> {
        self.update_block_stack()?;

        if self.has_next() {
            let item = self.leaf_block.children[self.index[self.level - 1]];
            self.index[self.level - 1] += 1;
            self.total_index += 1;

            Ok(Some(item))
        } else {
            Ok(None)
        }
    }
}
