use super::*;
use crate::disasm::address::MemoryRegion;

#[test]
fn sjis_lead_byte_ranges() {
    assert!(is_sjis_lead_byte(0x81));
    assert!(is_sjis_lead_byte(0x9F));
    assert!(is_sjis_lead_byte(0xE0));
    assert!(is_sjis_lead_byte(0xEF));
    assert!(!is_sjis_lead_byte(0x80));
    assert!(!is_sjis_lead_byte(0xA0));
    assert!(!is_sjis_lead_byte(0xF0));
}

#[test]
fn sjis_trail_byte_ranges() {
    assert!(is_sjis_trail_byte(0x40));
    assert!(is_sjis_trail_byte(0x7E));
    assert!(is_sjis_trail_byte(0x80));
    assert!(is_sjis_trail_byte(0xFC));
    assert!(!is_sjis_trail_byte(0x3F));
    assert!(!is_sjis_trail_byte(0x7F)); // 0x7F is NOT a valid trail byte
    assert!(!is_sjis_trail_byte(0xFD));
}

#[test]
fn looks_like_text_ascii() {
    let mut space = AddressSpace::new();
    // "Hello" + null
    space.add_region(MemoryRegion::new(
        "test",
        0x1000,
        b"Hello\x00".to_vec(),
    ));
    assert!(TextRefAnalyzer::looks_like_text(&space, 0x1000));
}

#[test]
fn looks_like_text_sjis() {
    let mut space = AddressSpace::new();
    // 3 Shift-JIS chars: U+3042 (あ) = 0x82 0xA0 in SJIS, repeated 3 times + null
    let data = vec![0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0x00];
    space.add_region(MemoryRegion::new("test", 0x1000, data));
    assert!(TextRefAnalyzer::looks_like_text(&space, 0x1000));
}

#[test]
fn looks_like_text_too_short() {
    let mut space = AddressSpace::new();
    // Only 2 chars -> below threshold
    space.add_region(MemoryRegion::new("test", 0x1000, b"Hi\x00".to_vec()));
    assert!(!TextRefAnalyzer::looks_like_text(&space, 0x1000));
}

#[test]
fn looks_like_text_binary_data() {
    let mut space = AddressSpace::new();
    // Random binary data (control chars)
    space.add_region(MemoryRegion::new(
        "test",
        0x1000,
        vec![0x01, 0x02, 0x03, 0x04],
    ));
    assert!(!TextRefAnalyzer::looks_like_text(&space, 0x1000));
}

#[test]
fn xref_summary() {
    let mut db = AnalysisDb::new();
    db.add_xref(XRef {
        from: 0x1000,
        to: 0x2000,
        kind: XRefKind::Call,
    });
    db.add_xref(XRef {
        from: 0x1004,
        to: 0x2000,
        kind: XRefKind::Branch,
    });
    db.add_xref(XRef {
        from: 0x1008,
        to: 0x3000,
        kind: XRefKind::LiteralPoolRef,
    });

    let summary = XRefReport::summary(&db);
    assert_eq!(summary.total, 3);
    assert_eq!(summary.calls, 1);
    assert_eq!(summary.branches, 1);
    assert_eq!(summary.literal_pool_refs, 1);
}

#[test]
fn format_xrefs_to_address() {
    let mut db = AnalysisDb::new();
    db.add_xref(XRef {
        from: 0x1000,
        to: 0x2000,
        kind: XRefKind::Call,
    });
    db.add_xref(XRef {
        from: 0x1004,
        to: 0x2000,
        kind: XRefKind::Branch,
    });

    let formatted = XRefReport::format_xrefs_to(&db, 0x2000);
    assert!(formatted.contains("call@00001000"));
    assert!(formatted.contains("branch@00001004"));
}

#[test]
fn format_xrefs_empty() {
    let db = AnalysisDb::new();
    let formatted = XRefReport::format_xrefs_to(&db, 0x9999);
    assert!(formatted.is_empty());
}

#[test]
fn find_text_pointers() {
    let mut db = AnalysisDb::new();
    // A literal pool entry pointing to text in Work RAM Low
    db.mark_literal_pool(0x5000, 0x0020_1000);

    let mut space = AddressSpace::new();
    // Put valid SJIS text at the target address
    let text_data = vec![0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0x00]; // あいう\0
    space.add_region(MemoryRegion::new("ram_l", 0x0020_1000, text_data));

    let candidates = TextRefAnalyzer::find_potential_text_pointers(&db, &space);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].text_addr, 0x0020_1000);
    assert_eq!(candidates[0].pointer_addr, 0x5000);
}

#[test]
fn find_text_pointers_ignores_non_text() {
    let mut db = AnalysisDb::new();
    db.mark_literal_pool(0x5000, 0x0020_1000);

    let mut space = AddressSpace::new();
    // Binary data, not text
    space.add_region(MemoryRegion::new(
        "ram_l",
        0x0020_1000,
        vec![0x01, 0x02, 0x03, 0x04],
    ));

    let candidates = TextRefAnalyzer::find_potential_text_pointers(&db, &space);
    assert_eq!(candidates.len(), 0);
}
