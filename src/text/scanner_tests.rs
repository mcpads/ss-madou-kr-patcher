use super::*;

#[test]
fn scan_ascii_string() {
    let data = b"\x00\x00Hello World\x00\x00";
    let matches = scan_strings(data, 3);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].text, "Hello World");
    assert_eq!(matches[0].offset, 2);
    assert_eq!(matches[0].char_count, 11);
}

#[test]
fn scan_sjis_hiragana() {
    // あいう = 0x82A0 0x82A2 0x82A4 in Shift-JIS
    let data = vec![0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0x00];
    let matches = scan_strings(&data, 3);
    assert_eq!(matches.len(), 1);
    assert!(matches[0].text.contains('あ'));
    assert!(matches[0].text.contains('い'));
    assert!(matches[0].text.contains('う'));
    assert_eq!(matches[0].char_count, 3);
}

#[test]
fn scan_min_chars_filters_short() {
    let data = b"Hi\x00There\x00";
    let matches = scan_strings(data, 3);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].text, "There");
}

#[test]
fn scan_multiple_strings() {
    let data = b"Hello\x00World\x00Test\x00";
    let matches = scan_strings(data, 3);
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].text, "Hello");
    assert_eq!(matches[1].text, "World");
    assert_eq!(matches[2].text, "Test");
}

#[test]
fn scan_mixed_ascii_sjis() {
    // "ABC" + あ(0x82A0) + "XY" + null
    let mut data = b"ABC".to_vec();
    data.extend_from_slice(&[0x82, 0xA0]);
    data.extend_from_slice(b"XY\x00");
    let matches = scan_strings(&data, 3);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].char_count, 6); // A, B, C, あ, X, Y
}

#[test]
fn scan_binary_noise_skipped() {
    let data = vec![0x01, 0x02, 0x03, 0xFF, 0xFE, 0xFD];
    let matches = scan_strings(&data, 1);
    assert!(matches.is_empty());
}
