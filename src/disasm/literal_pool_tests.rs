use super::*;

#[test]
fn interpret_hardware_register() {
    let db = AnalysisDb::new();
    let result = LiteralPoolAnalyzer::interpret(0x05F8_0000, &db);
    assert_eq!(
        result,
        LiteralPoolInterpretation::HardwareRegister(0x05F8_0000, "VDP2_TVMD")
    );
}

#[test]
fn interpret_code_pointer() {
    let mut db = AnalysisDb::new();
    db.mark_function(0x0600_4000, Some("main".into()));
    let result = LiteralPoolAnalyzer::interpret(0x0600_4000, &db);
    assert_eq!(result, LiteralPoolInterpretation::CodePointer(0x0600_4000));
}

#[test]
fn interpret_data_pointer_work_ram_high() {
    let db = AnalysisDb::new();
    // Even address in Work RAM High, but not classified as code -> data pointer
    let result = LiteralPoolAnalyzer::interpret(0x0600_8000, &db);
    assert_eq!(result, LiteralPoolInterpretation::DataPointer(0x0600_8000));
}

#[test]
fn interpret_data_pointer_work_ram_low() {
    let db = AnalysisDb::new();
    let result = LiteralPoolAnalyzer::interpret(0x0020_1000, &db);
    assert_eq!(result, LiteralPoolInterpretation::DataPointer(0x0020_1000));
}

#[test]
fn interpret_constant() {
    let db = AnalysisDb::new();
    let result = LiteralPoolAnalyzer::interpret(0x0000_00FF, &db);
    assert_eq!(result, LiteralPoolInterpretation::Constant(0x0000_00FF));
}

#[test]
fn interpret_odd_work_ram_high_as_constant() {
    let db = AnalysisDb::new();
    // Odd address in Work RAM High range -> not a valid code/data pointer
    let result = LiteralPoolAnalyzer::interpret(0x0600_4001, &db);
    assert_eq!(result, LiteralPoolInterpretation::Constant(0x0600_4001));
}

#[test]
fn hardware_register_priority_over_range() {
    // SMPC registers are in a range that could match other heuristics
    let db = AnalysisDb::new();
    let result = LiteralPoolAnalyzer::interpret(0x0010_0001, &db);
    assert_eq!(
        result,
        LiteralPoolInterpretation::HardwareRegister(0x0010_0001, "SMPC_IREG0")
    );
}

#[test]
fn classify_all_stats() {
    let mut db = AnalysisDb::new();
    db.mark_function(0x0600_4000, None);
    db.mark_literal_pool(0x1000, 0x0600_4000); // code pointer
    db.mark_literal_pool(0x1004, 0x0020_1000); // data pointer
    db.mark_literal_pool(0x1008, 0x05F8_0000); // hardware reg
    db.mark_literal_pool(0x100C, 0x0000_00FF); // constant

    let stats = LiteralPoolAnalyzer::classify_all(&db);
    assert_eq!(stats.total, 4);
    assert_eq!(stats.code_pointers, 1);
    assert_eq!(stats.data_pointers, 1);
    assert_eq!(stats.hardware_regs, 1);
    assert_eq!(stats.constants, 1);
}
