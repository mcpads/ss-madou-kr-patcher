use super::*;
use crate::disasm::analysis::AnalysisDb;

#[test]
fn listing_header() {
    let db = AnalysisDb::new();
    let writer = ListingWriter::new(&db);
    let mut buf = Vec::new();
    writer.write_listing(&mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("Madou Monogatari"));
    assert!(output.contains("Functions: 0"));
}

#[test]
fn listing_with_function() {
    let mut db = AnalysisDb::new();
    db.mark_function(0x1000, Some("test_func".into()));

    // Add a simple instruction
    use crate::disasm::linear::DisasmLine;
    use crate::sh2;
    let inst = sh2::decode(0x0009); // NOP
    db.instructions.insert(
        0x1000,
        DisasmLine {
            addr: 0x1000,
            opcode: 0x0009,
            instruction: inst,
            branch_target: None,
            literal_pool_addr: None,
            literal_pool_value: None,
        },
    );

    let writer = ListingWriter::new(&db);
    let mut buf = Vec::new();
    writer.write_listing(&mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("test_func"));
    assert!(output.contains("00001000"));
    assert!(output.contains("NOP"));
}
