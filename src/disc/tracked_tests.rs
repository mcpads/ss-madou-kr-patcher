use super::*;
use crate::disc::sector::{RAW_SECTOR_SIZE, SYNC_PATTERN, USER_DATA_SIZE};

/// Build a minimal valid disc image with `n` sectors.
fn make_test_disc(n: usize) -> DiscImage {
    let mut data = vec![0u8; n * RAW_SECTOR_SIZE];
    for i in 0..n {
        let offset = i * RAW_SECTOR_SIZE;
        data[offset..offset + 12].copy_from_slice(&SYNC_PATTERN);
        // MSF header (doesn't matter for writes, just needs valid sync)
        data[offset + 12] = 0x00; // minutes
        data[offset + 13] = 0x02; // seconds
        data[offset + 14] = i as u8; // frames
        data[offset + 15] = 0x01; // mode 1
    }
    DiscImage::from_bytes(data).unwrap()
}

#[test]
fn new_tracked_disc_has_no_regions() {
    let disc = make_test_disc(10);
    let tracked = TrackedDisc::new(disc);
    assert_eq!(tracked.region_count(), 0);
    assert!(tracked.check().is_ok());
}

#[test]
fn read_delegation_works() {
    let disc = make_test_disc(10);
    let tracked = TrackedDisc::new(disc);
    assert_eq!(tracked.sector_count(), 10);
    let sector = tracked.read_sector(0).unwrap();
    assert_eq!(sector.mode, 1);
    let ud = tracked.read_user_data(0).unwrap();
    assert_eq!(ud.len(), USER_DATA_SIZE);
}

#[test]
fn write_registers_single_sector() {
    let disc = make_test_disc(10);
    let mut tracked = TrackedDisc::new(disc);
    let data = [0xAA; 100];
    tracked
        .write_user_data_at(3, &data, "test:single")
        .unwrap();
    assert_eq!(tracked.region_count(), 1);

    // Verify data was actually written
    let ud = tracked.read_user_data(3).unwrap();
    assert_eq!(&ud[..100], &data[..]);
    // Remainder zero-padded
    assert!(ud[100..].iter().all(|&b| b == 0));
}

#[test]
fn write_file_registers_multiple_sectors() {
    let disc = make_test_disc(20);
    let mut tracked = TrackedDisc::new(disc);
    let data = vec![0xBB; USER_DATA_SIZE * 3 + 500]; // 3.x sectors
    let sectors = tracked.write_file_at(5, &data, "test:multi").unwrap();
    assert_eq!(sectors, 4);
    assert_eq!(tracked.region_count(), 1);
}

#[test]
fn overlapping_writes_detected() {
    let disc = make_test_disc(20);
    let mut tracked = TrackedDisc::new(disc);
    let data_a = vec![0xAA; USER_DATA_SIZE * 5];
    let data_b = vec![0xBB; USER_DATA_SIZE * 3];
    tracked.write_file_at(10, &data_a, "font").unwrap(); // 10..15
    tracked.write_file_at(13, &data_b, "seq").unwrap(); // 13..16
    let err = tracked.check().unwrap_err();
    assert!(err.contains("OVERLAP"));
    assert!(err.contains("font"));
    assert!(err.contains("seq"));
}

#[test]
fn non_overlapping_writes_ok() {
    let disc = make_test_disc(30);
    let mut tracked = TrackedDisc::new(disc);
    let data_a = vec![0xAA; USER_DATA_SIZE * 5];
    let data_b = vec![0xBB; USER_DATA_SIZE * 3];
    tracked.write_file_at(10, &data_a, "font").unwrap(); // 10..15
    tracked.write_file_at(15, &data_b, "seq").unwrap(); // 15..18
    assert!(tracked.check().is_ok());
}

#[test]
fn label_preserved_in_tracker() {
    let disc = make_test_disc(10);
    let mut tracked = TrackedDisc::new(disc);
    tracked
        .write_user_data_at(0, &[1, 2, 3], "ISO9660:FONT.CEL")
        .unwrap();
    // No public way to inspect labels directly from TrackedDisc,
    // but check() passing confirms registration
    assert_eq!(tracked.region_count(), 1);
    assert!(tracked.check().is_ok());
}

#[test]
fn into_inner_returns_disc() {
    let disc = make_test_disc(5);
    let tracked = TrackedDisc::new(disc);
    let inner = tracked.into_inner();
    assert_eq!(inner.sector_count(), 5);
}

#[test]
fn extract_file_delegates() {
    let disc = make_test_disc(10);
    let mut tracked = TrackedDisc::new(disc);
    // Write known data then extract
    let data = vec![0xCC; 100];
    tracked
        .write_user_data_at(2, &data, "test")
        .unwrap();
    let extracted = tracked.extract_file(2, 100).unwrap();
    assert_eq!(&extracted[..], &data[..]);
}

#[test]
fn out_of_range_write_errors() {
    let disc = make_test_disc(5);
    let mut tracked = TrackedDisc::new(disc);
    let result = tracked.write_user_data_at(10, &[0], "oob");
    assert!(result.is_err());
}
