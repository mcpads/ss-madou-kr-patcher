use super::*;

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
    assert!(!is_sjis_trail_byte(0x7F));
    assert!(!is_sjis_trail_byte(0xFD));
}

#[test]
fn printable_ascii_ranges() {
    assert!(is_printable_ascii(0x20)); // space
    assert!(is_printable_ascii(0x41)); // 'A'
    assert!(is_printable_ascii(0x7E)); // '~'
    assert!(!is_printable_ascii(0x1F));
    assert!(!is_printable_ascii(0x7F)); // DEL
}

#[test]
fn sjis_single_covers_both() {
    assert!(is_sjis_single(0x41)); // ASCII 'A'
    assert!(is_sjis_single(0xA1)); // half-width katakana
    assert!(!is_sjis_single(0x81)); // lead byte, not single
    assert!(!is_sjis_single(0x00)); // null
}
