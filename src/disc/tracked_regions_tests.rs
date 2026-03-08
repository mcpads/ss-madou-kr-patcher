use super::*;

#[test]
fn empty_tracker_check_ok() {
    let tracker = SectorRegionTracker::new();
    assert!(tracker.check().is_ok());
    assert_eq!(tracker.len(), 0);
}

#[test]
fn single_region_check_ok() {
    let mut tracker = SectorRegionTracker::new();
    tracker.register(100, 10, "font");
    assert!(tracker.check().is_ok());
    assert_eq!(tracker.len(), 1);
}

#[test]
fn adjacent_regions_no_overlap() {
    let mut tracker = SectorRegionTracker::new();
    tracker.register(100, 10, "font"); // 100..110
    tracker.register(110, 5, "seq");   // 110..115
    assert!(tracker.check().is_ok());
}

#[test]
fn overlapping_regions_detected() {
    let mut tracker = SectorRegionTracker::new();
    tracker.register(100, 10, "font"); // 100..110
    tracker.register(105, 5, "seq");   // 105..110
    let err = tracker.check().unwrap_err();
    assert!(err.contains("OVERLAP"));
    assert!(err.contains("font"));
    assert!(err.contains("seq"));
}

#[test]
fn contained_region_detected() {
    let mut tracker = SectorRegionTracker::new();
    tracker.register(100, 20, "big");   // 100..120
    tracker.register(105, 5, "small");  // 105..110
    let err = tracker.check().unwrap_err();
    assert!(err.contains("OVERLAP"));
}

#[test]
fn non_overlapping_unordered_ok() {
    let mut tracker = SectorRegionTracker::new();
    tracker.register(200, 10, "b");
    tracker.register(100, 10, "a");
    tracker.register(300, 10, "c");
    assert!(tracker.check().is_ok());
}

#[test]
fn zero_length_ignored() {
    let mut tracker = SectorRegionTracker::new();
    tracker.register(100, 0, "empty");
    assert_eq!(tracker.len(), 0);
    assert!(tracker.check().is_ok());
}

#[test]
fn collision_report_contains_all_overlaps() {
    let mut tracker = SectorRegionTracker::new();
    tracker.register(100, 10, "a"); // 100..110
    tracker.register(109, 10, "b"); // 109..119
    tracker.register(118, 10, "c"); // 118..128
    let err = tracker.check().unwrap_err();
    assert!(err.contains("2")); // 2 collisions
}

#[test]
fn same_label_overlap_allowed() {
    let mut tracker = SectorRegionTracker::new();
    tracker.register(100, 5, "ISO9660:rootdir"); // 100..105
    tracker.register(100, 5, "ISO9660:rootdir"); // 100..105 (same sector, same label)
    assert!(tracker.check().is_ok());
}

#[test]
fn label_preserved() {
    let mut tracker = SectorRegionTracker::new();
    tracker.register(0, 1, "ISO9660:FONT.CEL");
    assert_eq!(tracker.regions()[0].label, "ISO9660:FONT.CEL");
}

#[test]
fn transitive_containment_detected() {
    // A(100..200) contains B(110..115) and C(150..160).
    // B sits between A and C in sort order — C must still detect overlap with A.
    let mut tracker = SectorRegionTracker::new();
    tracker.register(100, 100, "big");    // 100..200
    tracker.register(110, 5, "small_a");  // 110..115
    tracker.register(150, 10, "small_b"); // 150..160
    let err = tracker.check().unwrap_err();
    assert!(err.contains("OVERLAP"));
    assert!(err.contains("small_b"));
    assert!(err.contains("big"));
}

#[test]
fn display_format() {
    let region = SectorRegion {
        start_lba: 0x100,
        sector_count: 16,
        label: "test".to_string(),
    };
    let s = format!("{}", region);
    assert!(s.contains("0x0100"));
    assert!(s.contains("16 sectors"));
    assert!(s.contains("[test]"));
}
