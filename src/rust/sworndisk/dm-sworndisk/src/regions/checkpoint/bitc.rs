use crate::{
    prelude::*,
    regions::{IndirectBlock, IndirectRecord, BIT},
    utils::*,
};

use crate::regions::*;

use core::iter::Rev;
use core::slice::Iter;

#[derive(Debug)]
pub struct BITRootMeta {
    /// unique ID of BIT
    pub unique_id: u64,

    /// on-disk crypt info
    pub record: IndirectRecord,

    /// BIT level
    pub level: usize,

    /// element number of BIT
    pub size: usize,
}

const BIT_ROOT_META_SIZE: usize = mem::size_of::<BITRootMeta>();

impl Serialize for BITRootMeta {
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();
        vec.try_extend_from_slice(&unsafe { mem::transmute::<u64, [u8; 8]>(self.unique_id) })?;
        vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(self.level) })?;
        vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(self.size) })?;
        vec.try_extend_from_slice(&self.record.serialize()?)?;
        vec.try_resize(BIT_ROOT_META_SIZE, 0u8)?;

        Ok(vec)
    }
}

impl Deserialize for BITRootMeta {
    fn deserialize(buf: &[u8]) -> Result<BITRootMeta> {
        if buf.len() != BIT_ROOT_META_SIZE {
            return Err(EINVAL);
        }

        let unique_id = unsafe { mem::transmute::<[u8; 8], u64>(buf[0..8].try_into().unwrap()) };
        let level = unsafe { mem::transmute::<[u8; 8], usize>(buf[8..16].try_into().unwrap()) };
        let size = unsafe { mem::transmute::<[u8; 8], usize>(buf[16..24].try_into().unwrap()) };
        let record =
            IndirectRecord::deserialize(buf[24..24 + INDIRECT_RECORD_SIZE].try_into().unwrap())?;

        Ok(Self {
            size,
            unique_id,
            level,
            record,
        })
    }
}

impl BITRootMeta {
    /// Check a LBA in the range of this BIT
    pub fn contains(&self, lba: u64) -> bool {
        let lba_range = self.record.lba_range;
        lba_range.0 <= lba && lba <= lba_range.1
    }

    /// Read BIT from disk
    pub fn read_from_disk(
        &self,
        aead: &Pin<Box<Aead>>,
        bdev: &BlockDevice,
        client: &DmIoClient,
        cache: &mut LruCache<u64, IndirectBlock>,
    ) -> Result<BIT> {
        let record = self.record.record;
        let root = match cache.get(&record.hba) {
            Some(root_block) => root_block.clone(),
            _ => {
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

                let root = IndirectBlock::deserialize(&block)?;
                cache.put(record.hba, root.clone())?;

                root
            }
        };

        Ok(BIT {
            root,
            size: self.size,
            record: self.record.clone(),
            level: self.level,
        })
    }
}

/// Block Index Table Category (BITC)
#[derive(Debug)]
pub struct BITCategory {
    /// Current unique ID for BIT root node
    pub bit_unique_id: u64,

    /// Root node vector
    pub category: Vec<Vec<BITRootMeta>>,
}

impl Serialize for BITCategory {
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();
        vec.try_extend_from_slice(&unsafe { mem::transmute::<u64, [u8; 8]>(self.bit_unique_id) })?;

        for i in 0..self.category.len() {
            let len = self.category[i].len();
            vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(len) })?;
            for item in self.category[i].iter() {
                vec.try_extend_from_slice(&item.serialize()?)?;
            }
        }

        Ok(vec)
    }
}

impl Deserialize for BITCategory {
    fn deserialize(buf: &[u8]) -> Result<Self> {
        let mut category = Vec::new();
        for _level in 0..LSM_TREE_MAX_LEVEL {
            category.try_push(Vec::new())?;
        }

        let bit_unique_id =
            unsafe { mem::transmute::<[u8; 8], u64>(buf[0..8].try_into().unwrap()) };
        let mut index = 8;
        for level in 0..LSM_TREE_MAX_LEVEL {
            let len = unsafe {
                mem::transmute::<[u8; 8], usize>(buf[index..index + 8].try_into().unwrap())
            };
            index += 8;

            for _i in 0..len {
                let root_meta = BITRootMeta::deserialize(&buf[index..index + BIT_ROOT_META_SIZE])?;
                category[level].try_push(root_meta)?;
                index += BIT_ROOT_META_SIZE;
            }
        }

        Ok(Self {
            bit_unique_id,
            category,
        })
    }
}

impl BITCategory {
    /// Create a new BITCategory
    pub fn new() -> Result<Self> {
        let mut category = Vec::new();
        for _ in 0..LSM_TREE_MAX_LEVEL {
            category.try_push(Vec::new())?;
        }

        Ok(Self {
            category,
            bit_unique_id: 0,
        })
    }

    /// Get the number of BIT
    pub fn len(&self) -> usize {
        let mut len = 0;
        for per_level_category in &self.category {
            len += per_level_category.len()
        }
        len
    }

    /// Add a BIT into BITCategory
    pub fn add_bit(&mut self, bit: BIT, level: usize) -> Result {
        if level >= LSM_TREE_MAX_LEVEL {
            return Err(EINVAL);
        }

        let meta_info = BITRootMeta {
            unique_id: self.bit_unique_id,
            record: bit.record,
            level: bit.level,
            size: bit.size(),
        };

        self.bit_unique_id += 1;
        self.category[level].try_push(meta_info)?;

        Ok(())
    }

    /// Get the number of BIT in a certain level
    pub fn level_size(&self, level: usize) -> usize {
        self.category[level].len()
    }

    /// Get the root metainfo of a BIT through its unique_id
    pub fn get_bit(&self, level: usize, index: usize) -> Option<&BITRootMeta> {
        if index >= self.category[level].len() {
            return None;
        }
        Some(&self.category[level][index])
    }

    /// Remove a BIT through its unique_id
    pub fn release_bit(&mut self, level: usize, unique_id: u64) -> Result {
        let mut index = None;
        let len = self.category[level].len();
        for i in 0..len {
            if self.category[level][i].unique_id == unique_id {
                index = Some(i);
            }
        }

        match index {
            Some(idx) => {
                self.category[level].remove(idx);
                Ok(())
            }
            None => Err(EINVAL),
        }
    }

    /// Reversely iterate the BIT of a certain level
    pub fn iter_level(&self, level: usize) -> Result<Rev<Iter<'_, BITRootMeta>>> {
        if level >= self.category.len() {
            return Err(EINVAL);
        }

        Ok(self.category[level].iter().rev())
    }

    /// Check whether to start a major compaction job
    pub fn is_compaction_required(&self) -> bool {
        for level in 0..self.category.len() {
            if self.category[level].len() >= MAX_COMPACTION_NUMBER {
                return true;
            }
        }
        return false;
    }
}
