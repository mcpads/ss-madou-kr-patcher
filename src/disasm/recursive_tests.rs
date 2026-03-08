use super::*;
use crate::disasm::address::MemoryRegion;

fn make_space(base: VAddr, data: &[u8]) -> AddressSpace {
    let mut space = AddressSpace::new();
    space.add_region(MemoryRegion::new("test", base, data.to_vec()));
    space
}

#[test]
fn simple_linear_block() {
    // NOP; NOP; RTS; NOP (delay slot)
    let data = [
        0x00, 0x09, // NOP
        0x00, 0x09, // NOP
        0x00, 0x0B, // RTS
        0x00, 0x09, // NOP (delay slot)
    ];
    let space = make_space(0x1000, &data);
    let mut disasm = RecursiveDisassembler::new(&space);
    disasm.add_entry_point(0x1000, Some("test_func".into()));
    let db = disasm.run();

    assert_eq!(db.functions.len(), 1);
    assert!(db.functions.contains(&0x1000));
    // All 4 instructions should be marked as code
    assert_eq!(db.code_count(), 4);
    assert_eq!(db.labels[&0x1000], "test_func");
}

#[test]
fn conditional_branch_both_paths() {
    // BT +2 (target = PC+4+2*2 = +8); NOP; RTS; NOP; NOP; RTS; NOP
    //                                                ^-- branch target
    let data = [
        0x89, 0x02, // BT disp=2 -> target = 0x1000+4+4 = 0x1008
        0x00, 0x09, // NOP (fall-through)
        0x00, 0x0B, // RTS
        0x00, 0x09, // NOP (delay slot)
        0x00, 0x0B, // RTS (branch target at 0x1008)
        0x00, 0x09, // NOP (delay slot)
    ];
    let space = make_space(0x1000, &data);
    let mut disasm = RecursiveDisassembler::new(&space);
    disasm.add_entry_point(0x1000, None);
    let db = disasm.run();

    // Both paths visited: all 6 instructions
    assert_eq!(db.code_count(), 6);
    // XRef from BT to target
    let xrefs = db.xrefs_to(0x1008);
    assert_eq!(xrefs.len(), 1);
    assert_eq!(xrefs[0].kind, XRefKind::ConditionalBranch);
}

#[test]
fn bsr_creates_function() {
    // BSR disp=2 -> target = 0x1000+4+4 = 0x1008
    // NOP (delay slot)
    // RTS; NOP
    // --- at 0x1008:
    // RTS; NOP
    let data = [
        0xB0, 0x02, // BSR disp=2 -> 0x1008
        0x00, 0x09, // NOP (delay slot)
        0x00, 0x0B, // RTS (fall-through at 0x1004)
        0x00, 0x09, // NOP (delay slot)
        0x00, 0x0B, // RTS (0x1008, called function)
        0x00, 0x09, // NOP (delay slot)
    ];
    let space = make_space(0x1000, &data);
    let mut disasm = RecursiveDisassembler::new(&space);
    disasm.add_entry_point(0x1000, None);
    let db = disasm.run();

    // Two functions: entry point + BSR target
    assert_eq!(db.functions.len(), 2);
    assert!(db.functions.contains(&0x1008));
    assert!(db.labels.contains_key(&0x1008));

    let xrefs = db.xrefs_to(0x1008);
    assert_eq!(xrefs.len(), 1);
    assert_eq!(xrefs[0].kind, XRefKind::Call);
}

#[test]
fn literal_pool_reference() {
    // MOV.L @(0,PC), R1   -> pool at (0x1000 & ~3)+4+0 = 0x1004
    // JSR @R1
    // NOP (delay slot)
    // pool: 0x06002000
    // Use disp=1 so pool = 0x1000 + 4 + 1*4 = 0x1008
    let data = [
        0xD1, 0x01, // MOV.L @(4+4,PC), R1   pool = (0x1000&~3)+4+4 = 0x1008
        0x41, 0x0B, // JSR @R1
        0x00, 0x09, // NOP (delay slot)
        0x00, 0x09, // NOP (fall-through after JSR)
        0x06, 0x00, 0x20, 0x00, // literal pool at 0x1008: 0x06002000
    ];

    let mut space = AddressSpace::new();
    space.add_region(MemoryRegion::new("code", 0x1000, data.to_vec()));
    // Add a target region so the function is reachable
    space.add_region(MemoryRegion::new("target", 0x0600_2000, vec![
        0x00, 0x0B, // RTS
        0x00, 0x09, // NOP
    ]));

    let mut disasm = RecursiveDisassembler::new(&space);
    disasm.add_entry_point(0x1000, None);
    let db = disasm.run();

    // Literal pool should be recorded
    assert_eq!(db.literal_pool_values.get(&0x1008), Some(&0x06002000));

    // JSR @R1 should resolve to 0x06002000 via backtracking
    assert!(db.functions.contains(&0x0600_2000));
}

#[test]
fn unconditional_branch_no_fallthrough() {
    // BRA disp=1 -> 0x1000+4+2 = 0x1006
    // NOP (delay slot)
    // 0xFF 0xFF (should NOT be decoded - no fall-through)
    // NOP (branch target at 0x1006)
    // RTS; NOP
    let data = [
        0xA0, 0x01, // BRA disp=1 -> 0x1006
        0x00, 0x09, // NOP (delay slot at 0x1002)
        0xFF, 0xFF, // unreachable data at 0x1004
        0x00, 0x09, // NOP (target at 0x1006)
        0x00, 0x0B, // RTS at 0x1008
        0x00, 0x09, // NOP (delay slot at 0x100A)
    ];
    let space = make_space(0x1000, &data);
    let mut disasm = RecursiveDisassembler::new(&space);
    disasm.add_entry_point(0x1000, None);
    let db = disasm.run();

    // 0x1004 should NOT be visited (unreachable)
    assert!(!db.instructions.contains_key(&0x1004));
    // But 0x1006 should be visited (branch target)
    assert!(db.instructions.contains_key(&0x1006));
}

#[test]
fn is_plausible_code_addr_test() {
    assert!(is_plausible_code_addr(0x0600_0000));
    assert!(is_plausible_code_addr(0x0600_4000));
    assert!(!is_plausible_code_addr(0x0600_4001)); // odd
    assert!(!is_plausible_code_addr(0x0700_0000)); // out of range
    assert!(!is_plausible_code_addr(0x0000_0000)); // BIOS ROM
}
