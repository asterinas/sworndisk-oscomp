/* Target features */
/// Any table that contains an instance of this target must have only one.
pub const DM_TARGET_SINGLETON: u64 = 1;
/// Indicates that a target does not support read-only devices.
pub const DM_TARGET_ALWAYS_WRITEABLE: u64 = 2;
/// Any device that contains a table with an instance of this target may never
/// have tables containing any different target type.
pub const DM_TARGET_IMMUTABLE: u64 = 4;
/// Indicates that a target may replace any target; even immutable targets.
/// .map, .map_rq, .clone_and_map_rq and .release_clone_rq are all defined.
pub const DM_TARGET_WILDCARD: u64 = 8;
/// A target implements own bio data integrity.
pub const DM_TARGET_INTEGRITY: u64 = 16;
/// A target passes integrity data to the lower device.
pub const DM_TARGET_PASSES_INTEGRITY: u64 = 32;
/// the target also supports host-managed zoned block devices but does not
/// support combining different zoned models.
pub const DM_TARGET_ZONED_HM: u64 = 64;
/// A target handles REQ_NOWAIT
pub const DM_TARGET_NOWAIT: u64 = 128;
/// A target supports passing through inline crypto support.
pub const DM_TARGET_PASSES_CRYPTO: u64 = 256;
/// DM_TARGET_MIXED_ZONED_MODEL
pub const DM_TARGET_MIXED_ZONED_MODEL: u64 = 512;

/* Device Mapper queue mode */
/// None
pub const DM_TYPE_NONE: u32 = 0;
/// Bio-based
pub const DM_TYPE_BIO_BASED: u32 = 1;
/// Request-based
pub const DM_TYPE_REQUEST_BASED: u32 = 2;
/// Dax-bio-based
pub const DM_TYPE_DAX_BIO_BASED: u32 = 3;

/* Return values from target end_io function */
/// done
pub const DM_ENDIO_DONE: u32 = 0;
/// incomplete
pub const DM_ENDIO_INCOMPLETE: u32 = 1;
/// requeue
pub const DM_ENDIO_REQUEUE: u32 = 2;
/// delay_requeue
pub const DM_ENDIO_DELAY_REQUEUE: u32 = 3;

#[repr(u32)]
/// status_type
pub enum StatusType {
    /// STATUSTYPE_INFO
    INFO,
    /// STATUSTYPE_TABLE
    TABLE,
    /// STATUSTYPE_IMA
    IMA,
}

#[repr(u16)]
#[allow(non_camel_case_types)]
/// Bio flags
pub enum BioFlags {
    /// don't put release vec pages
    BIO_NO_PAGE_REF,
    /// doesn't own data
    BIO_CLONED,
    /// bio is a bounce bio
    BIO_BOUNCED,
    /// contains userspace workingset pages
    BIO_WORKINGSET,
    /// Make BIO Quiet
    BIO_QUIET,
    /// chained bio, ->bi_remaining in effect
    BIO_CHAIN,
    /// bio has elevated ->bi_cnt
    BIO_REFFED,
    /// This bio has already been subjected to throttling rules. Don't do it again.
    BIO_THROTTLED,
    ///  bio_endio() should trace the final completion of this bio.
    BIO_TRACE_COMPLETION,
    /// has been accounted to a cgroup
    BIO_CGROUP_ACCT,
    /// set if bio goes through the rq_qos path
    BIO_TRACKED,
    /// Remapped
    BIO_REMAPPED,
    /// Owns a zoned device zone write lock
    BIO_ZONE_WRITE_LOCKED,
    /// can participate in per-cpu alloc cache
    BIO_PERCPU_CACHE,
    /// last bit
    BIO_FLAG_LAST,
}

#[repr(u16)]
/// bio operation directions
pub enum Direction {
    /// Read operation
    READ = 0,
    /// Write operation
    WRITE = 1,
    /// Unknown operation
    INVALID,
}

impl From<i32> for Direction {
    fn from(dir: i32) -> Direction {
        match dir {
            0 => Direction::READ,
            1 => Direction::WRITE,
            _ => Direction::INVALID,
        }
    }
}

/// bio operation types
#[repr(u32)]
#[allow(non_camel_case_types, missing_docs)]
#[derive(Debug)]
pub enum BioOperationType {
    READ = 0,
    WRITE = 1,
    FLUSH = 2,
    DISCARD = 3,
    SECURE_ERASE = 5,
    WRITE_SAME = 7,
    WRITE_ZEROES = 9,
    ZONE_OPEN = 10,
    ZONE_CLOSE = 11,
    ZONE_FINISH = 12,
    ZONE_APPEND = 13,
    ZONE_RESET = 15,
    ZONE_RESET_ALL = 17,
    DRY_IN = 34,
    DRY_OUT = 35,
    LAST = 36,
}
