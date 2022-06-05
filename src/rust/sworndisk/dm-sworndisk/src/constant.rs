pub use crate::regions::SWORNDISK_RECORD_SIZE;

/// Per block size (default 4KB, unit: Byte)
pub const BLOCK_SIZE: u64 = 4096;

/// Bytes of a sector in Linux (512 B)
pub const SECTOR_SIZE: u64 = 512;

/// How many blocks can a segment contains (default 1024 blocks)
pub const SEGMENT_BLOCK_NUMBER: u64 = 1024;

/// Shift bits of a sector
pub const SECTOR_SHIFT: u64 = 9;

/// Sector number of a block. For a 4KB block, the number of sectors is 8.
pub const BLOCK_SECTORS: u64 = BLOCK_SIZE / SECTOR_SIZE;

/// Per segment size (default 4MB, unit: Byte)
pub const SEGMENT_SIZE: u64 = BLOCK_SIZE * SEGMENT_BLOCK_NUMBER;

/// Segment sector number
pub const SEGMENT_SECTORS: u64 = SEGMENT_BLOCK_NUMBER * BLOCK_SECTORS;

/// Numbers of concurrent lock for dm_block_manager
pub const MAX_CONCURRENT_LOCKS: u32 = 5;

/* SuperBlock parameters */
/// Magic number of SwornDisk superblock
pub const SWORNDISK_MAGIC_NUMBER: u64 = 0x03070612;

/// Checksum XOR number of SuerBlock
pub const SWORNDISK_SUPERBLOCK_XOR: u32 = 998244353;

/// Position of 2 superblocks
pub const SWORNDISK_FIRST_SUPERBLOCK_LOCATION: u64 = 0;
pub const SWORNDISK_SECOND_SUPERBLOCK_LOCATION: u64 = 1;

/* Encrypt parameters */
/// AES-128-GCM key length
pub const SWORNDISK_KEY_LENGTH: usize = 16;

/// AES-128-GCM nonce (iv) length
pub const SWORNDISK_NONCE_LENGTH: usize = 12;

/// AES-128-GCM MAC (tag) length
pub const SWORNDISK_MAC_LENGTH: usize = 16;

/* I/O operation types */
/// READ (b00)
pub const READ: u32 = 0;
/// WRITE (b01)
pub const WRITE: u32 = 1;
/// FLUSH (b10)
pub const FLUSH: u32 = 2;

/* BIT parameters */
/// Max levels of a BIT
pub const BIT_MAX_LEVEL: usize = 5;

/// Max levels of dsLSM-tree
pub const LSM_TREE_MAX_LEVEL: usize = 5;

/// Max record number of MemTable
pub const MEMTABLE_THRESHOLD: usize = 65536;

/// Max size of IndirectBlock or LeafBlock LRU Cache
pub const LRU_CACHE_MAX_SIZE: usize = 4096;
