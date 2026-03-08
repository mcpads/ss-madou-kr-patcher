use super::*;

#[test]
fn mark_code_and_function() {
    let mut db = AnalysisDb::new();
    db.mark_code(0x1000);
    db.mark_function(0x2000, Some("main".into()));

    assert_eq!(db.addr_types[&0x1000], AddrType::Code);
    assert_eq!(db.addr_types[&0x2000], AddrType::FunctionEntry);
    assert!(db.functions.contains(&0x2000));
    assert_eq!(db.labels[&0x2000], "main");
}

#[test]
fn mark_function_auto_label() {
    let mut db = AnalysisDb::new();
    db.mark_function(0x06004000, None);
    assert_eq!(db.labels[&0x06004000], "sub_06004000");
}

#[test]
fn mark_code_does_not_override_function() {
    let mut db = AnalysisDb::new();
    db.mark_function(0x1000, None);
    db.mark_code(0x1000);
    assert_eq!(db.addr_types[&0x1000], AddrType::FunctionEntry);
}

#[test]
fn xrefs_roundtrip() {
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

    assert_eq!(db.xrefs_to(0x2000).len(), 2);
    assert_eq!(db.xrefs_from(0x1000).len(), 1);
    assert_eq!(db.xrefs_to(0x9999).len(), 0);
}

#[test]
fn literal_pool() {
    let mut db = AnalysisDb::new();
    db.mark_literal_pool(0x3000, 0xDEADBEEF);
    assert_eq!(db.addr_types[&0x3000], AddrType::LiteralPool);
    assert_eq!(db.literal_pool_values[&0x3000], 0xDEADBEEF);
}

#[test]
fn code_count() {
    let mut db = AnalysisDb::new();
    db.mark_code(0x1000);
    db.mark_code(0x1002);
    db.mark_function(0x2000, None);
    db.mark_literal_pool(0x3000, 0);
    assert_eq!(db.code_count(), 3); // 2 code + 1 function entry
}
