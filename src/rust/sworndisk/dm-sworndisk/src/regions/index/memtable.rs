use crate::{prelude::*, regions::index::record::Record};
use core::cmp;
use kernel::rbtree::{RBTree, RBTreeIterator, RBTreeNode};

/// MemTable: Level-0 (in memory) block index table
pub struct MemTable {
    size: usize,
    lba_range: (u64, u64),
    inner: RBTree<u64, Record>,
}

impl Debug for MemTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemTable").finish()
    }
}

impl MemTable {
    /// Create a in-memory index
    pub fn new() -> Self {
        Self {
            size: 0,
            lba_range: (u64::MAX, u64::MIN),
            inner: RBTree::new(),
        }
    }

    /// Insert a record into the index
    pub fn insert(&mut self, lba: u64, record: Record) -> Result<Option<RBTreeNode<u64, Record>>> {
        self.size += 1;
        self.lba_range.0 = cmp::min(self.lba_range.0, lba);
        self.lba_range.1 = cmp::max(self.lba_range.1, lba);
        self.inner.try_insert(lba, record)
    }

    /// Find a record from the index
    pub fn find(&self, lba: u64) -> Option<&Record> {
        if lba < self.lba_range.0 || lba > self.lba_range.1 {
            return None;
        }

        self.inner.get(&lba)
    }

    pub fn iter(&self) -> RBTreeIterator<'_, u64, Record> {
        self.inner.iter()
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn clear(&mut self) {
        self.size = 0;
        self.inner = RBTree::new();
    }
}
