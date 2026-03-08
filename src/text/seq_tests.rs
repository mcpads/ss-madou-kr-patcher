use super::*;

#[test]
fn detect_simple_offset_table() {
    // Two entries pointing past the table area
    let mut data = vec![0; 64];
    // Entry 0: offset 0 -> target 0x0010 (16)
    data[0] = 0x00;
    data[1] = 0x10;
    // Entry 1: offset 2 -> target 0x0020 (32)
    data[2] = 0x00;
    data[3] = 0x20;
    // Entry 2: 0x0000 (terminator)
    data[4] = 0x00;
    data[5] = 0x00;

    let table = detect_offset_table(&data);
    assert_eq!(table.len(), 2);
    assert_eq!(table[0].table_offset, 0);
    assert_eq!(table[0].target_offset, 0x10);
    assert_eq!(table[1].table_offset, 2);
    assert_eq!(table[1].target_offset, 0x20);
}

#[test]
fn detect_no_table_in_binary_noise() {
    // Data that doesn't form a valid table: starts with a backwards pointer
    let data = vec![0xFF, 0x01, 0x00, 0x02, 0x03, 0x04];
    let table = detect_offset_table(&data);
    assert!(table.is_empty());
}

#[test]
fn detect_empty_data() {
    let table = detect_offset_table(&[]);
    assert!(table.is_empty());
}

#[test]
fn analyze_seq_with_text() {
    let mut data = vec![0u8; 128];
    // Put some text at offset 64
    let text = b"Hello World";
    data[64..64 + text.len()].copy_from_slice(text);
    data[64 + text.len()] = 0x00; // null terminator

    let analysis = analyze_seq(&data);
    assert_eq!(analysis.size, 128);
    assert!(analysis.strings.iter().any(|s| s.text == "Hello World"));
}
