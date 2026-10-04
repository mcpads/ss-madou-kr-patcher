use super::*;
use crate::disasm::address::MemoryRegion;

fn make_test_space(base: VAddr, data: &[u8]) -> AddressSpace {
    let mut space = AddressSpace::new();
    space.add_region(MemoryRegion::new("test", base, data.to_vec()));
    space
}

#[test]
fn linear_disasm_nop_sequence() {
    // 4 NOPs (0x0009)
    let data = [0x00, 0x09, 0x00, 0x09, 0x00, 0x09, 0x00, 0x09];
    let space = make_test_space(0x0600_4000, &data);
    let lines = disassemble_linear(&space, 0x0600_4000, 0x0600_4008);

    assert_eq!(lines.len(), 4);
    for line in &lines {
        assert_eq!(line.opcode, 0x0009);
        assert!(line.branch_target.is_none());
        assert!(line.literal_pool_addr.is_none());
    }
    assert_eq!(lines[0].addr, 0x0600_4000);
    assert_eq!(lines[3].addr, 0x0600_4006);
}

#[test]
fn linear_disasm_branch_target() {
    // BRA +4 (0xA002) at 0x06004000: target = PC + 4 + 2*2 = 0x06004008
    let data = [0xA0, 0x02, 0x00, 0x09]; // BRA +4, NOP (delay slot)
    let space = make_test_space(0x0600_4000, &data);
    let lines = disassemble_linear(&space, 0x0600_4000, 0x0600_4004);

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].branch_target, Some(0x0600_4008));
}

#[test]
fn linear_disasm_literal_pool() {
    // MOV.L @(0,PC), R1 (0xD100) at 0x06004000
    // target = (PC & ~3) + 4 + 0*4 = 0x06004004
    // Then place a 32-bit value at 0x06004004
    let data = vec![0xD1, 0x00, 0x00, 0x09, 0x06, 0x01, 0x02, 0x34];
    let space = make_test_space(0x0600_4000, &data);
    let lines = disassemble_linear(&space, 0x0600_4000, 0x0600_4004);

    assert_eq!(lines[0].literal_pool_addr, Some(0x0600_4004));
    assert_eq!(lines[0].literal_pool_value, Some(0x06010234));
}

#[test]
fn word_literal_uses_only_its_two_bytes_and_sign_extends() {
    for (word, expected) in [(0x0FFFu16, 0x0000_0FFF), (0x8000, 0xFFFF_8000)] {
        // MOV.W @(0,PC),R3; NOP; word literal; unrelated next word.
        let [hi, lo] = word.to_be_bytes();
        let space = make_test_space(0x1000, &[0x93, 0, 0, 9, hi, lo, 0, 0x80]);
        let lines = disassemble_linear(&space, 0x1000, 0x1002);
        assert_eq!(lines[0].literal_pool_value, Some(expected));
    }
}

#[test]
fn word_literal_at_region_end_is_readable() {
    let space = make_test_space(0x1000, &[0x93, 0, 0, 9, 0x0F, 0xFF]);
    let lines = disassemble_linear(&space, 0x1000, 0x1002);
    assert_eq!(lines[0].literal_pool_value, Some(0x0FFF));
}

#[test]
fn mova_references_an_address_without_loading_its_contents() {
    let space = make_test_space(0x1000, &[0xC7, 0, 0, 9, 0xDE, 0xAD, 0xBE, 0xEF]);
    let lines = disassemble_linear(&space, 0x1000, 0x1002);
    assert_eq!(lines[0].literal_pool_addr, Some(0x1004));
    assert_eq!(lines[0].literal_pool_value, None);
}

#[test]
fn linear_disasm_stops_at_end() {
    let data = [0x00, 0x09]; // single NOP
    let space = make_test_space(0x1000, &data);
    let lines = disassemble_linear(&space, 0x1000, 0x2000); // end way past data

    assert_eq!(lines.len(), 1);
}

#[test]
fn disasm_line_display() {
    let data = [0x00, 0x09]; // NOP
    let space = make_test_space(0x0600_4000, &data);
    let lines = disassemble_linear(&space, 0x0600_4000, 0x0600_4002);

    let output = format!("{}", lines[0]);
    assert!(output.contains("06004000"));
    assert!(output.contains("0009"));
    assert!(output.contains("NOP"));
}

#[test]
fn linear_disasm_with_literal_pool_display() {
    let data = vec![0xD1, 0x00, 0x00, 0x09, 0xDE, 0xAD, 0xBE, 0xEF];
    let space = make_test_space(0x0600_4000, &data);
    let lines = disassemble_linear(&space, 0x0600_4000, 0x0600_4002);

    let output = format!("{}", lines[0]);
    assert!(output.contains("=0xDEADBEEF"));
}
