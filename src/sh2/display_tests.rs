use super::super::decode::decode;

#[test]
fn test_display_known_opcodes() {
    let cases: &[(u16, &str)] = &[
        (0x0009, "NOP"),
        (0x000B, "RTS"),
        (0x002B, "RTE"),
        (0xE500, "MOV     #0,R5"),
        (0xD103, "MOV.L   @(12,PC),R1"),
        (0x410B, "JSR     @R1"),
        (0x8900, "BT      0"),
        (0x8BFE, "BF      -4"),
        (0xA000, "BRA     0"),
        (0xB100, "BSR     512"),
        (0x3128, "SUB     R2,R1"),
        (0x312C, "ADD     R2,R1"),
    ];
    for &(opcode, expected_asm) in cases {
        let inst = decode(opcode);
        let actual = format!("{}", inst);
        assert_eq!(
            actual, expected_asm,
            "Failed for opcode 0x{:04X}: expected '{}', got '{}'",
            opcode, expected_asm, actual
        );
    }
}

#[test]
fn test_display_mov_imm_negative() {
    let inst = decode(0xE3FF); // MOV #-1, R3
    assert_eq!(format!("{}", inst), "MOV     #-1,R3");
}

#[test]
fn test_display_add_imm_negative() {
    let inst = decode(0x7FFC); // ADD #-4, R15
    assert_eq!(format!("{}", inst), "ADD     #-4,R15");
}

#[test]
fn test_display_unknown() {
    let inst = decode(0xF123); // Unknown (SH-4 FPU)
    assert_eq!(format!("{}", inst), ".word   0xF123");
}

#[test]
fn test_display_mov_reg() {
    let inst = decode(0x6353); // MOV R5, R3
    assert_eq!(format!("{}", inst), "MOV     R5,R3");
}

#[test]
fn test_display_jmp() {
    let inst = decode(0x432B); // JMP @R3
    assert_eq!(format!("{}", inst), "JMP     @R3");
}

#[test]
fn test_display_shifts() {
    assert_eq!(format!("{}", decode(0x4100)), "SHLL    R1");
    assert_eq!(format!("{}", decode(0x4101)), "SHLR    R1");
    assert_eq!(format!("{}", decode(0x4108)), "SHLL2   R1");
    assert_eq!(format!("{}", decode(0x4118)), "SHLL8   R1");
    assert_eq!(format!("{}", decode(0x4128)), "SHLL16  R1");
}

#[test]
fn test_display_system_regs() {
    assert_eq!(format!("{}", decode(0x002A)), "STS     PR,R0");
    assert_eq!(format!("{}", decode(0x4122)), "STS.L   PR,@-R1");
    assert_eq!(format!("{}", decode(0x4126)), "LDS.L   @R1+,PR");
    assert_eq!(format!("{}", decode(0x412A)), "LDS     R1,PR");
}

#[test]
fn test_display_mov_l_disp() {
    // MOV.L R0, @(4,R1) -> 1101
    assert_eq!(format!("{}", decode(0x1101)), "MOV.L   R0,@(4,R1)");
    // MOV.L @(8,R2), R3 -> 5322
    assert_eq!(format!("{}", decode(0x5322)), "MOV.L   @(8,R2),R3");
}

#[test]
fn test_display_mov_stores_loads() {
    assert_eq!(format!("{}", decode(0x2100)), "MOV.B   R0,@R1");
    assert_eq!(format!("{}", decode(0x2101)), "MOV.W   R0,@R1");
    assert_eq!(format!("{}", decode(0x2102)), "MOV.L   R0,@R1");
    assert_eq!(format!("{}", decode(0x6320)), "MOV.B   @R2,R3");
    assert_eq!(format!("{}", decode(0x6321)), "MOV.W   @R2,R3");
    assert_eq!(format!("{}", decode(0x6322)), "MOV.L   @R2,R3");
}

#[test]
fn test_display_predec_postinc() {
    assert_eq!(format!("{}", decode(0x2104)), "MOV.B   R0,@-R1");
    assert_eq!(format!("{}", decode(0x6324)), "MOV.B   @R2+,R3");
}

#[test]
fn test_display_branch_negative() {
    // BT/S -2 -> 8DFF
    assert_eq!(format!("{}", decode(0x8DFF)), "BT/S    -2");
    // BF/S -6 -> 8FFD
    assert_eq!(format!("{}", decode(0x8FFD)), "BF/S    -6");
}

#[test]
fn test_display_bra_negative() {
    // BRA -2 -> AFFF (disp=-1, *2=-2)
    assert_eq!(format!("{}", decode(0xAFFF)), "BRA     -2");
}

#[test]
fn test_display_gbr() {
    assert_eq!(format!("{}", decode(0xC005)), "MOV.B   R0,@(5,GBR)");
    assert_eq!(format!("{}", decode(0xC405)), "MOV.B   @(5,GBR),R0");
    assert_eq!(format!("{}", decode(0xC105)), "MOV.W   R0,@(10,GBR)");
    assert_eq!(format!("{}", decode(0xC205)), "MOV.L   R0,@(20,GBR)");
}

#[test]
fn test_display_mova() {
    assert_eq!(format!("{}", decode(0xC70A)), "MOVA    @(40,PC),R0");
}

#[test]
fn test_display_trapa() {
    assert_eq!(format!("{}", decode(0xC320)), "TRAPA   #32");
}

#[test]
fn test_display_imm_logical() {
    assert_eq!(format!("{}", decode(0xC9FF)), "AND     #255,R0");
    assert_eq!(format!("{}", decode(0xCB0F)), "OR      #15,R0");
    assert_eq!(format!("{}", decode(0xCA55)), "XOR     #85,R0");
    assert_eq!(format!("{}", decode(0xC8AA)), "TST     #170,R0");
}

#[test]
fn test_display_cmp_eq_imm() {
    assert_eq!(format!("{}", decode(0x8800)), "CMP/EQ  #0,R0");
    assert_eq!(format!("{}", decode(0x88FF)), "CMP/EQ  #-1,R0");
}

#[test]
fn test_display_mac() {
    assert_eq!(format!("{}", decode(0x012F)), "MAC.L   @R2+,@R1+");
    assert_eq!(format!("{}", decode(0x412F)), "MAC.W   @R2+,@R1+");
}

#[test]
fn test_display_mov_w_pc_rel() {
    // MOV.W @(10,PC), R2 -> 9205
    assert_eq!(format!("{}", decode(0x9205)), "MOV.W   @(10,PC),R2");
}

#[test]
fn test_display_all_opcodes_no_panic() {
    // Verify Display formatting doesn't panic for any opcode
    for opcode in 0u16..=0xFFFF {
        let inst = decode(opcode);
        let _ = format!("{}", inst);
    }
}

/// Test a representative SH-2 function prologue/epilogue pattern.
#[test]
fn test_display_prologue_epilogue() {
    // Typical prologue:
    //   STS.L   PR, @-R15     (4F22)
    //   MOV.L   R14, @-R15    (2FE6)
    //   MOV     R15, R14      (6EF3)
    //   ADD     #-16, R15     (7FF0)
    assert_eq!(format!("{}", decode(0x4F22)), "STS.L   PR,@-R15");
    assert_eq!(format!("{}", decode(0x2FE6)), "MOV.L   R14,@-R15");
    assert_eq!(format!("{}", decode(0x6EF3)), "MOV     R15,R14");
    assert_eq!(format!("{}", decode(0x7FF0)), "ADD     #-16,R15");

    // Typical epilogue:
    //   MOV     R14, R15      (6FE3)
    //   MOV.L   @R15+, R14   (6EF6)
    //   LDS.L   @R15+, PR    (4F26)
    //   RTS                   (000B)
    //   NOP                   (0009)
    assert_eq!(format!("{}", decode(0x6FE3)), "MOV     R14,R15");
    assert_eq!(format!("{}", decode(0x6EF6)), "MOV.L   @R15+,R14");
    assert_eq!(format!("{}", decode(0x4F26)), "LDS.L   @R15+,PR");
    assert_eq!(format!("{}", decode(0x000B)), "RTS");
    assert_eq!(format!("{}", decode(0x0009)), "NOP");
}
