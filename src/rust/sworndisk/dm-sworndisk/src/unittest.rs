/// Kernel Module Unittest Solution
///
/// Since the kernel module workspace does not support `cargo test` command to run
/// a unit test, we separate a indepenedent module to write and run unit tests.
use crate::{prelude::*, utils::*};

fn test_get_lba_range() {
    pr_info!("[TEST] utils::get_lba_range()");

    assert_eq!(get_lba_range(0, 4096), (0, 1, 0, 0));
    assert_eq!(get_lba_range(0, 2048), (0, 1, 0, 2048));
    assert_eq!(get_lba_range(0, 0), (0, 1, 0, 4096));
    assert_eq!(get_lba_range(4, 2048), (0, 1, 2048, 0));

    pr_info!("[TEST] utils::get_lba_range() PASSED");
}

fn test_bitmap() {
    pr_info!("[TEST] utils::BitMap");

    let mut bitmap = BitMap::new(4096).unwrap();

    assert_eq!(bitmap.is_full(), false);
    assert_eq!(bitmap.is_empty(), true);
    bitmap.set_bit(0).unwrap();
    bitmap.set_bit(1).unwrap();
    assert_eq!(bitmap.get_bit(0).unwrap(), true);
    assert_eq!(bitmap.get_bit(1).unwrap(), true);
    assert_eq!(bitmap.get_bit(2).unwrap(), false);
    assert_eq!(bitmap.get_first_zero_bit().unwrap(), 2);
    assert_eq!(bitmap.is_empty(), false);
    bitmap.clear_bit(0).unwrap();
    assert_eq!(bitmap.get_bit(0).unwrap(), false);
    assert_eq!(bitmap.get_bit(1).unwrap(), true);
    assert_eq!(bitmap.get_first_zero_bit().unwrap(), 0);
    bitmap.set_bit(8).unwrap();
    assert_eq!(bitmap.get_bit(8).unwrap(), true);

    pr_info!("[TEST] utils::BitMap PASSED");
}

fn test_serialize() {
    let record = crate::regions::index::Record {
        hba: 1,
        key: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5],
        nonce: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1],
        mac: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5],
    };

    let buffer = record.serialize().unwrap();
    pr_info!("Record buffer: {:?}", buffer);

    let record_restore = crate::regions::index::Record::deserialize(&buffer[..]).unwrap();
    pr_info!("Record: {:?}", record_restore);
}

/// Run all unit tests
pub fn run_all_test() {
    pr_warn!("[TEST] Running SwornDisk kernel module unit tests");

    test_get_lba_range();
    test_bitmap();
    test_serialize();

    pr_warn!("[TEST] All unit tests are passed. Continue to load SwornDisk module.");
}
