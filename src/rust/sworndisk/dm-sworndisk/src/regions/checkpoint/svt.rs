use crate::{prelude::*, utils::*};

/// Segment Validity Table (SVT)
#[derive(Debug)]
pub struct SVT(BitMap);

impl SVT {
    /// Create a new SVT that at least contains `n` segments.
    pub fn new(n_segments: u64) -> Result<Self> {
        let bitmap = BitMap::new(n_segments as usize)?;
        Ok(SVT(bitmap))
    }

    /// Check the SVT is full
    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }

    /// Get the length
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Alloc a new segment and mark its index as used in BitMap.
    /// Returns the index (HBA) of the segment.
    pub fn alloc(&mut self) -> Result<usize> {
        // No more free segment to alloc, returns ENOSPC
        if self.0.is_full() {
            // TODO: segment cleaning @kirainmoe
            return Err(ENOSPC);
        }

        let index = self.0.get_first_zero_bit()?;
        self.0.set_bit(index)?;
        Ok(index)
    }

    /// Release a segment by index (HBA).
    pub fn release(&mut self, index: u64) -> Result {
        self.0.clear_bit(index as usize)?;
        Ok(())
    }
}

impl Serialize for SVT {
    fn serialize(&self) -> Result<Vec<u8>> {
        self.0.serialize()
    }
}

impl Deserialize for SVT {
    fn deserialize(buf: &[u8]) -> Result<Self> {
        let bitmap = BitMap::deserialize(buf)?;

        Ok(SVT(bitmap))
    }
}
