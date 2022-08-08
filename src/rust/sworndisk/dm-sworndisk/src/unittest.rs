/// Kernel Module Unittest Solution
///
/// Since the kernel module workspace does not support `cargo test` command to run
/// a unit test, we separate a indepenedent module to write and run unit tests.
use crate::{prelude::*, utils::*};

// test utils::get_lba_range()
fn test_get_lba_range() {
    assert_eq!(get_lba_range(0, 4096), (0, 1, 0, 0));
    assert_eq!(get_lba_range(0, 2048), (0, 1, 0, 2048));
    assert_eq!(get_lba_range(0, 0), (0, 1, 0, 4096));
    assert_eq!(get_lba_range(4, 2048), (0, 1, 2048, 0));
}

// test utils::BitMap
fn test_bitmap() {
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
}

/// Run all unit tests
pub fn run_all_test() {
    pr_warn!("[TEST] Running SwornDisk kernel module unit tests");

    test_get_lba_range();
    test_bitmap();

    pr_warn!("[TEST] All unit tests are passed. Continue to load SwornDisk module.");
}
