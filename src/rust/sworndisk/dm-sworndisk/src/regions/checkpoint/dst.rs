use crate::{prelude::*, utils::*};

/// Data Segment Table (DST)
#[derive(Debug)]
pub struct DST {
    /// Block Validity Map (BVM)
    bvm: BitMap,
    /// Last modification timestamp
    last_modify: u64,
}

impl DST {
    /// Create a new DST
    pub fn new() -> Result<Self> {
        let bvm = BitMap::new(SEGMENT_BLOCK_NUMBER as usize)?;
        Ok(Self {
            bvm,
            last_modify: current_timestamp(),
        })
    }

    /// Check the BVM is full
    pub fn is_full(&self) -> bool {
        self.bvm.is_full()
    }

    /// Alloc a new block and mark as used
    pub fn alloc_block(&mut self) -> Result<usize> {
        let index = self.bvm.get_first_zero_bit()?;
        self.bvm.set_bit(index)?;
        self.last_modify = current_timestamp();

        Ok(index)
    }

    /// Set a block as used
    pub fn set_block(&mut self, index: usize) -> Result {
        self.bvm.set_bit(index)?;
        self.last_modify = current_timestamp();

        Ok(())
    }

    /// Release a block
    pub fn clear_block(&mut self, index: usize) -> Result {
        self.bvm.clear_bit(index)?;
        self.last_modify = current_timestamp();

        Ok(())
    }
}

impl Serialize for DST {
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();

        let bvm = self.bvm.serialize()?;
        let bvm_len = bvm.len();

        vec.try_extend_from_slice(&unsafe { mem::transmute::<u64, [u8; 8]>(self.last_modify) })?;
        vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(bvm_len) })?;
        vec.try_extend_from_slice(&bvm)?;

        Ok(vec)
    }
}

impl Deserialize for DST {
    fn deserialize(buf: &[u8]) -> Result<Self> {
        let last_modify = unsafe { mem::transmute::<[u8; 8], u64>(buf[0..8].try_into().unwrap()) };
        let bvm_len = unsafe { mem::transmute::<[u8; 8], usize>(buf[8..16].try_into().unwrap()) };
        if bvm_len + 16 != buf.len() {
            return Err(EINVAL);
        }
        let bvm = BitMap::deserialize(&buf[16..16 + bvm_len])?;
        Ok(Self { bvm, last_modify })
    }
}
