use super::*;

// ===== Sign extension tests =====

#[test]
fn test_sign_ext8() {
    assert_eq!(sign_ext8(0x00), 0);
    assert_eq!(sign_ext8(0x7F), 127);
    assert_eq!(sign_ext8(0x80), -128);
    assert_eq!(sign_ext8(0xFF), -1);
    assert_eq!(sign_ext8(0xFE), -2);
}

#[test]
fn test_sign_ext12() {
    assert_eq!(sign_ext12(0x000), 0);
    assert_eq!(sign_ext12(0x7FF), 2047);
    assert_eq!(sign_ext12(0x800), -2048);
    assert_eq!(sign_ext12(0xFFF), -1);
    assert_eq!(sign_ext12(0xFFE), -2);
    assert_eq!(sign_ext12(0x100), 256);
}

// ===== Known opcode decoding tests =====

#[test]
fn test_decode_nop() {
    let inst = decode(0x0009);
    assert_eq!(inst.kind, InstructionKind::Nop);
    assert_eq!(inst.flow, FlowKind::Normal);
    assert_eq!(inst.opcode, 0x0009);
}

#[test]
fn test_decode_rts() {
    let inst = decode(0x000B);
    assert_eq!(inst.kind, InstructionKind::Rts);
    assert_eq!(inst.flow, FlowKind::Return);
}

#[test]
fn test_decode_rte() {
    let inst = decode(0x002B);
    assert_eq!(inst.kind, InstructionKind::Rte);
    assert_eq!(inst.flow, FlowKind::ExceptionReturn);
}

#[test]
fn test_decode_clrt() {
    let inst = decode(0x0008);
    assert_eq!(inst.kind, InstructionKind::ClrT);
}

#[test]
fn test_decode_sett() {
    let inst = decode(0x0018);
    assert_eq!(inst.kind, InstructionKind::SetT);
}

#[test]
fn test_decode_clrmac() {
    let inst = decode(0x0028);
    assert_eq!(inst.kind, InstructionKind::ClrMac);
}

#[test]
fn test_decode_div0u() {
    let inst = decode(0x0019);
    assert_eq!(inst.kind, InstructionKind::Div0U);
}

#[test]
fn test_decode_sleep() {
    let inst = decode(0x001B);
    assert_eq!(inst.kind, InstructionKind::Sleep);
}

#[test]
fn test_decode_mov_imm() {
    // MOV #0, R5 -> E500
    let inst = decode(0xE500);
    assert_eq!(
        inst.kind,
        InstructionKind::MovImm {
            imm: 0,
            rn: Reg(5)
        }
    );

    // MOV #-1, R3 -> E3FF
    let inst = decode(0xE3FF);
    assert_eq!(
        inst.kind,
        InstructionKind::MovImm {
            imm: -1,
            rn: Reg(3)
        }
    );

    // MOV #127, R0 -> E07F
    let inst = decode(0xE07F);
    assert_eq!(
        inst.kind,
        InstructionKind::MovImm {
            imm: 127,
            rn: Reg(0)
        }
    );
}

#[test]
fn test_decode_mov_l_pc_rel() {
    // MOV.L @(12,PC), R1 -> D103
    let inst = decode(0xD103);
    assert_eq!(
        inst.kind,
        InstructionKind::MovLPcRel {
            disp: 3,
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_mov_w_pc_rel() {
    // MOV.W @(disp,PC), R2 -> 9205
    let inst = decode(0x9205);
    assert_eq!(
        inst.kind,
        InstructionKind::MovWPcRel {
            disp: 5,
            rn: Reg(2)
        }
    );
}

#[test]
fn test_decode_jsr() {
    // JSR @R1 -> 410B
    let inst = decode(0x410B);
    assert_eq!(inst.kind, InstructionKind::Jsr { rm: Reg(1) });
    assert_eq!(inst.flow, FlowKind::Call);
}

#[test]
fn test_decode_jmp() {
    // JMP @R3 -> 432B
    let inst = decode(0x432B);
    assert_eq!(inst.kind, InstructionKind::Jmp { rm: Reg(3) });
    assert_eq!(inst.flow, FlowKind::UnconditionalBranch);
}

#[test]
fn test_decode_bt() {
    // BT +0 -> 8900
    let inst = decode(0x8900);
    assert_eq!(inst.kind, InstructionKind::Bt { disp: 0 });
    assert_eq!(inst.flow, FlowKind::ConditionalBranch);
}

#[test]
fn test_decode_bf() {
    // BF -4 -> 8BFE (0xFE sign-extended = -2, displayed as disp*2=-4)
    let inst = decode(0x8BFE);
    assert_eq!(inst.kind, InstructionKind::Bf { disp: -2 });
    assert_eq!(inst.flow, FlowKind::ConditionalBranch);
}

#[test]
fn test_decode_bts() {
    // BT/S +6 -> 8D03
    let inst = decode(0x8D03);
    assert_eq!(inst.kind, InstructionKind::Bts { disp: 3 });
    assert_eq!(inst.flow, FlowKind::ConditionalBranchDelayed);
}

#[test]
fn test_decode_bfs() {
    // BF/S -2 -> 8FFF
    let inst = decode(0x8FFF);
    assert_eq!(inst.kind, InstructionKind::Bfs { disp: -1 });
    assert_eq!(inst.flow, FlowKind::ConditionalBranchDelayed);
}

#[test]
fn test_decode_bra() {
    // BRA +0 -> A000
    let inst = decode(0xA000);
    assert_eq!(inst.kind, InstructionKind::Bra { disp: 0 });
    assert_eq!(inst.flow, FlowKind::UnconditionalBranch);

    // BRA -1 -> AFFF (disp = 0xFFF sign-extended = -1)
    let inst = decode(0xAFFF);
    assert_eq!(inst.kind, InstructionKind::Bra { disp: -1 });
}

#[test]
fn test_decode_bsr() {
    // BSR +256 -> B100
    let inst = decode(0xB100);
    assert_eq!(inst.kind, InstructionKind::Bsr { disp: 0x100 });
    assert_eq!(inst.flow, FlowKind::Call);
}

#[test]
fn test_decode_add_reg() {
    // ADD R2, R1 -> 312C
    let inst = decode(0x312C);
    assert_eq!(
        inst.kind,
        InstructionKind::Add {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_sub() {
    // SUB R2, R1 -> 3128
    let inst = decode(0x3128);
    assert_eq!(
        inst.kind,
        InstructionKind::Sub {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_add_imm() {
    // ADD #10, R4 -> 740A
    let inst = decode(0x740A);
    assert_eq!(
        inst.kind,
        InstructionKind::AddImm {
            imm: 10,
            rn: Reg(4)
        }
    );

    // ADD #-4, R15 -> 7FFC
    let inst = decode(0x7FFC);
    assert_eq!(
        inst.kind,
        InstructionKind::AddImm {
            imm: -4,
            rn: Reg(15)
        }
    );
}

#[test]
fn test_decode_cmp_eq_imm() {
    // CMP/EQ #0, R0 -> 8800
    let inst = decode(0x8800);
    assert_eq!(inst.kind, InstructionKind::CmpEqImm { imm: 0 });
}

#[test]
fn test_decode_mov_reg() {
    // MOV R5, R3 -> 6353
    let inst = decode(0x6353);
    assert_eq!(
        inst.kind,
        InstructionKind::MovReg {
            rm: Reg(5),
            rn: Reg(3)
        }
    );
}

#[test]
fn test_decode_mov_l_store_disp() {
    // MOV.L R0, @(4,R1) -> 1101
    let inst = decode(0x1101);
    assert_eq!(
        inst.kind,
        InstructionKind::MovLDispStore {
            disp: 1,
            rm: Reg(0),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_mov_l_load_disp() {
    // MOV.L @(8,R2), R3 -> 5322
    let inst = decode(0x5322);
    assert_eq!(
        inst.kind,
        InstructionKind::MovLDispLoad {
            disp: 2,
            rm: Reg(2),
            rn: Reg(3)
        }
    );
}

#[test]
fn test_decode_mov_stores() {
    // MOV.B R0, @R1 -> 2100
    assert_eq!(
        decode(0x2100).kind,
        InstructionKind::MovBStore {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
    // MOV.W R0, @R1 -> 2101
    assert_eq!(
        decode(0x2101).kind,
        InstructionKind::MovWStore {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
    // MOV.L R0, @R1 -> 2102
    assert_eq!(
        decode(0x2102).kind,
        InstructionKind::MovLStore {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_mov_loads() {
    // MOV.B @R2, R3 -> 6320
    assert_eq!(
        decode(0x6320).kind,
        InstructionKind::MovBLoad {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
    // MOV.W @R2, R3 -> 6321
    assert_eq!(
        decode(0x6321).kind,
        InstructionKind::MovWLoad {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
    // MOV.L @R2, R3 -> 6322
    assert_eq!(
        decode(0x6322).kind,
        InstructionKind::MovLLoad {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
}

#[test]
fn test_decode_mov_predec() {
    // MOV.B R0, @-R1 -> 2104
    assert_eq!(
        decode(0x2104).kind,
        InstructionKind::MovBStorePreDec {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
    // MOV.W R0, @-R1 -> 2105
    assert_eq!(
        decode(0x2105).kind,
        InstructionKind::MovWStorePreDec {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
    // MOV.L R0, @-R1 -> 2106
    assert_eq!(
        decode(0x2106).kind,
        InstructionKind::MovLStorePreDec {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_mov_postinc() {
    // MOV.B @R2+, R3 -> 6324
    assert_eq!(
        decode(0x6324).kind,
        InstructionKind::MovBLoadPostInc {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
    // MOV.W @R2+, R3 -> 6325
    assert_eq!(
        decode(0x6325).kind,
        InstructionKind::MovWLoadPostInc {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
    // MOV.L @R2+, R3 -> 6326
    assert_eq!(
        decode(0x6326).kind,
        InstructionKind::MovLLoadPostInc {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
}

#[test]
fn test_decode_shifts() {
    assert_eq!(decode(0x4100).kind, InstructionKind::Shll { rn: Reg(1) });
    assert_eq!(decode(0x4101).kind, InstructionKind::Shlr { rn: Reg(1) });
    assert_eq!(decode(0x4120).kind, InstructionKind::Shal { rn: Reg(1) });
    assert_eq!(decode(0x4121).kind, InstructionKind::Shar { rn: Reg(1) });
    assert_eq!(decode(0x4108).kind, InstructionKind::Shll2 { rn: Reg(1) });
    assert_eq!(decode(0x4109).kind, InstructionKind::Shlr2 { rn: Reg(1) });
    assert_eq!(decode(0x4118).kind, InstructionKind::Shll8 { rn: Reg(1) });
    assert_eq!(decode(0x4119).kind, InstructionKind::Shlr8 { rn: Reg(1) });
    assert_eq!(decode(0x4128).kind, InstructionKind::Shll16 { rn: Reg(1) });
    assert_eq!(decode(0x4129).kind, InstructionKind::Shlr16 { rn: Reg(1) });
}

#[test]
fn test_decode_rotates() {
    assert_eq!(decode(0x4104).kind, InstructionKind::Rotl { rn: Reg(1) });
    assert_eq!(decode(0x4105).kind, InstructionKind::Rotr { rn: Reg(1) });
    assert_eq!(decode(0x4124).kind, InstructionKind::Rotcl { rn: Reg(1) });
    assert_eq!(decode(0x4125).kind, InstructionKind::Rotcr { rn: Reg(1) });
}

#[test]
fn test_decode_dt() {
    // DT R3 -> 4310
    assert_eq!(decode(0x4310).kind, InstructionKind::Dt { rn: Reg(3) });
}

#[test]
fn test_decode_cmp_pz_pl() {
    assert_eq!(decode(0x4311).kind, InstructionKind::CmpPz { rn: Reg(3) });
    assert_eq!(decode(0x4315).kind, InstructionKind::CmpPl { rn: Reg(3) });
}

#[test]
fn test_decode_logical() {
    assert_eq!(
        decode(0x2129).kind,
        InstructionKind::And {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x212B).kind,
        InstructionKind::Or {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x212A).kind,
        InstructionKind::Xor {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x2128).kind,
        InstructionKind::Tst {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x6127).kind,
        InstructionKind::Not {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_extensions() {
    assert_eq!(
        decode(0x610E).kind,
        InstructionKind::ExtsB {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x610F).kind,
        InstructionKind::ExtsW {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x610C).kind,
        InstructionKind::ExtuB {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x610D).kind,
        InstructionKind::ExtuW {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_swap_xtrct() {
    assert_eq!(
        decode(0x6128).kind,
        InstructionKind::SwapB {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x6129).kind,
        InstructionKind::SwapW {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x212D).kind,
        InstructionKind::Xtrct {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_compares() {
    assert_eq!(
        decode(0x3120).kind,
        InstructionKind::CmpEq {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x3122).kind,
        InstructionKind::CmpHs {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x3123).kind,
        InstructionKind::CmpGe {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x3126).kind,
        InstructionKind::CmpHi {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x3127).kind,
        InstructionKind::CmpGt {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x212C).kind,
        InstructionKind::CmpStr {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_system_register_ops() {
    // STC SR, R0 -> 0002
    assert_eq!(decode(0x0002).kind, InstructionKind::StcSr { rn: Reg(0) });
    // STC GBR, R0 -> 0012
    assert_eq!(decode(0x0012).kind, InstructionKind::StcGbr { rn: Reg(0) });
    // STC VBR, R0 -> 0022
    assert_eq!(decode(0x0022).kind, InstructionKind::StcVbr { rn: Reg(0) });

    // STS MACH, R0 -> 000A
    assert_eq!(
        decode(0x000A).kind,
        InstructionKind::StsMach { rn: Reg(0) }
    );
    // STS MACL, R0 -> 001A
    assert_eq!(
        decode(0x001A).kind,
        InstructionKind::StsMacl { rn: Reg(0) }
    );
    // STS PR, R0 -> 002A
    assert_eq!(decode(0x002A).kind, InstructionKind::StsPr { rn: Reg(0) });

    // LDS R1, MACH -> 410A
    assert_eq!(
        decode(0x410A).kind,
        InstructionKind::LdsMach { rm: Reg(1) }
    );
    // LDS R1, MACL -> 411A
    assert_eq!(
        decode(0x411A).kind,
        InstructionKind::LdsMacl { rm: Reg(1) }
    );
    // LDS R1, PR -> 412A
    assert_eq!(decode(0x412A).kind, InstructionKind::LdsPr { rm: Reg(1) });
}

#[test]
fn test_decode_system_register_stack_ops() {
    // STS.L MACH, @-Rn -> 4n02
    assert_eq!(
        decode(0x4102).kind,
        InstructionKind::StsLMach { rn: Reg(1) }
    );
    // STS.L MACL, @-Rn -> 4n12
    assert_eq!(
        decode(0x4112).kind,
        InstructionKind::StsLMacl { rn: Reg(1) }
    );
    // STS.L PR, @-Rn -> 4n22
    assert_eq!(
        decode(0x4122).kind,
        InstructionKind::StsLPr { rn: Reg(1) }
    );

    // LDS.L @Rm+, MACH -> 4n06
    assert_eq!(
        decode(0x4106).kind,
        InstructionKind::LdsLMach { rm: Reg(1) }
    );
    // LDS.L @Rm+, MACL -> 4n16
    assert_eq!(
        decode(0x4116).kind,
        InstructionKind::LdsLMacl { rm: Reg(1) }
    );
    // LDS.L @Rm+, PR -> 4n26
    assert_eq!(
        decode(0x4126).kind,
        InstructionKind::LdsLPr { rm: Reg(1) }
    );

    // STC.L SR, @-Rn -> 4n03
    assert_eq!(
        decode(0x4103).kind,
        InstructionKind::StcLSr { rn: Reg(1) }
    );
    // STC.L GBR, @-Rn -> 4n13
    assert_eq!(
        decode(0x4113).kind,
        InstructionKind::StcLGbr { rn: Reg(1) }
    );
    // STC.L VBR, @-Rn -> 4n23
    assert_eq!(
        decode(0x4123).kind,
        InstructionKind::StcLVbr { rn: Reg(1) }
    );

    // LDC.L @Rm+, SR -> 4n07
    assert_eq!(
        decode(0x4107).kind,
        InstructionKind::LdcLSr { rm: Reg(1) }
    );
    // LDC.L @Rm+, GBR -> 4n17
    assert_eq!(
        decode(0x4117).kind,
        InstructionKind::LdcLGbr { rm: Reg(1) }
    );
    // LDC.L @Rm+, VBR -> 4n27
    assert_eq!(
        decode(0x4127).kind,
        InstructionKind::LdcLVbr { rm: Reg(1) }
    );
}

#[test]
fn test_decode_ldc_direct() {
    // LDC R1, SR -> 410E
    assert_eq!(decode(0x410E).kind, InstructionKind::LdcSr { rm: Reg(1) });
    // LDC R1, GBR -> 411E
    assert_eq!(
        decode(0x411E).kind,
        InstructionKind::LdcGbr { rm: Reg(1) }
    );
    // LDC R1, VBR -> 412E
    assert_eq!(
        decode(0x412E).kind,
        InstructionKind::LdcVbr { rm: Reg(1) }
    );
}

#[test]
fn test_decode_gbr_ops() {
    // MOV.B R0, @(disp,GBR) -> C0dd
    assert_eq!(
        decode(0xC005).kind,
        InstructionKind::MovBGbr {
            disp: 5,
            store: true
        }
    );
    // MOV.B @(disp,GBR), R0 -> C4dd
    assert_eq!(
        decode(0xC405).kind,
        InstructionKind::MovBGbr {
            disp: 5,
            store: false
        }
    );
    // MOV.W R0, @(disp,GBR) -> C1dd
    assert_eq!(
        decode(0xC105).kind,
        InstructionKind::MovWGbr {
            disp: 5,
            store: true
        }
    );
    // MOV.L R0, @(disp,GBR) -> C2dd
    assert_eq!(
        decode(0xC205).kind,
        InstructionKind::MovLGbr {
            disp: 5,
            store: true
        }
    );
}

#[test]
fn test_decode_mova() {
    // MOVA @(disp,PC), R0 -> C7dd
    assert_eq!(decode(0xC70A).kind, InstructionKind::Mova { disp: 10 });
}

#[test]
fn test_decode_trapa() {
    // TRAPA #imm -> C3ii
    assert_eq!(decode(0xC320).kind, InstructionKind::Trapa { imm: 0x20 });
}

#[test]
fn test_decode_imm_logical() {
    assert_eq!(decode(0xC9FF).kind, InstructionKind::AndImm { imm: 0xFF });
    assert_eq!(decode(0xCB0F).kind, InstructionKind::OrImm { imm: 0x0F });
    assert_eq!(decode(0xCA55).kind, InstructionKind::XorImm { imm: 0x55 });
    assert_eq!(decode(0xC8AA).kind, InstructionKind::TstImm { imm: 0xAA });
}

#[test]
fn test_decode_r0_indexed() {
    // MOV.B R0, @(R0,R1) -> 0104
    assert_eq!(
        decode(0x0104).kind,
        InstructionKind::MovBR0StoreIndexed {
            rm: Reg(0),
            rn: Reg(1)
        }
    );
    // MOV.B @(R0,R1), R2 -> 021C
    assert_eq!(
        decode(0x021C).kind,
        InstructionKind::MovBR0LoadIndexed {
            rm: Reg(1),
            rn: Reg(2)
        }
    );
}

#[test]
fn test_decode_mul() {
    // MUL.L R2, R3 -> 0327
    assert_eq!(
        decode(0x0327).kind,
        InstructionKind::MulL {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
    // MULS.W R2, R3 -> 232F
    assert_eq!(
        decode(0x232F).kind,
        InstructionKind::MulS {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
    // MULU.W R2, R3 -> 232E
    assert_eq!(
        decode(0x232E).kind,
        InstructionKind::MulU {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
    // DMULS.L R2, R3 -> 332D
    assert_eq!(
        decode(0x332D).kind,
        InstructionKind::DmulS {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
    // DMULU.L R2, R3 -> 3325
    assert_eq!(
        decode(0x3325).kind,
        InstructionKind::DmulU {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
}

#[test]
fn test_decode_div() {
    // DIV0S R2, R3 -> 2327
    assert_eq!(
        decode(0x2327).kind,
        InstructionKind::Div0S {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
    // DIV1 R2, R3 -> 3324
    assert_eq!(
        decode(0x3324).kind,
        InstructionKind::Div1 {
            rm: Reg(2),
            rn: Reg(3)
        }
    );
}

#[test]
fn test_decode_mac() {
    // MAC.L @Rm+, @Rn+ -> 0nmF
    assert_eq!(
        decode(0x012F).kind,
        InstructionKind::MacL {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    // MAC.W @Rm+, @Rn+ -> 4nmF
    assert_eq!(
        decode(0x412F).kind,
        InstructionKind::MacW {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_neg() {
    assert_eq!(
        decode(0x612B).kind,
        InstructionKind::Neg {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x612A).kind,
        InstructionKind::Negc {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_addc_addv_subc_subv() {
    assert_eq!(
        decode(0x312E).kind,
        InstructionKind::Addc {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x312F).kind,
        InstructionKind::Addv {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x312A).kind,
        InstructionKind::Subc {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
    assert_eq!(
        decode(0x312B).kind,
        InstructionKind::Subv {
            rm: Reg(2),
            rn: Reg(1)
        }
    );
}

#[test]
fn test_decode_movt() {
    // MOVT R1 -> 0129
    assert_eq!(decode(0x0129).kind, InstructionKind::MovT { rn: Reg(1) });
}

#[test]
fn test_decode_unknown_f_group() {
    // All 0xFxxx should be Unknown on SH-2
    let inst = decode(0xF000);
    assert!(matches!(inst.kind, InstructionKind::Unknown { opcode: 0xF000 }));
}

#[test]
fn test_decode_8xxx_mov_b_disp_store() {
    // MOV.B R0, @(2,R3) -> 8032
    let inst = decode(0x8032);
    assert_eq!(
        inst.kind,
        InstructionKind::MovBDispStore {
            disp: 2,
            rn: Reg(3)
        }
    );
}

#[test]
fn test_decode_8xxx_mov_w_disp_store() {
    // MOV.W R0, @(3,R4) -> 8143
    let inst = decode(0x8143);
    assert_eq!(
        inst.kind,
        InstructionKind::MovWDispStore {
            disp: 3,
            rn: Reg(4)
        }
    );
}

#[test]
fn test_decode_8xxx_mov_b_disp_load() {
    // MOV.B @(1,R5), R0 -> 8451
    let inst = decode(0x8451);
    assert_eq!(
        inst.kind,
        InstructionKind::MovBDispLoad {
            disp: 1,
            rm: Reg(5)
        }
    );
}

#[test]
fn test_decode_8xxx_mov_w_disp_load() {
    // MOV.W @(2,R6), R0 -> 8562
    let inst = decode(0x8562);
    assert_eq!(
        inst.kind,
        InstructionKind::MovWDispLoad {
            disp: 2,
            rm: Reg(6)
        }
    );
}

#[test]
fn test_decode_byte_logical_gbr() {
    assert_eq!(decode(0xCD0F).kind, InstructionKind::AndB { imm: 0x0F });
    assert_eq!(decode(0xCF0F).kind, InstructionKind::OrB { imm: 0x0F });
    assert_eq!(decode(0xCE0F).kind, InstructionKind::XorB { imm: 0x0F });
    assert_eq!(decode(0xCC0F).kind, InstructionKind::TstB { imm: 0x0F });
}

// ===== Exhaustive decode test =====

#[test]
fn test_decode_all_opcodes_no_panic() {
    for opcode in 0u16..=0xFFFF {
        let inst = decode(opcode);
        // Just verify it doesn't panic and returns a valid opcode
        assert_eq!(inst.opcode, opcode);
    }
}

// ===== Branch target calculation tests =====

#[test]
fn test_branch_target_bt_positive() {
    let inst = decode(0x8905); // BT +5
    assert_eq!(inst.branch_target(0x06004000), Some(0x0600400E));
}

#[test]
fn test_branch_target_bf_negative_wrap() {
    let inst = decode(0x8BFD); // BF disp=-3
    // target = 0x100 + 4 + (-3)*2 = 0x100 + 4 - 6 = 0xFE
    assert_eq!(inst.branch_target(0x100), Some(0xFE));
}

#[test]
fn test_branch_target_bra_large_positive() {
    let inst = decode(0xA7FF); // BRA disp=0x7FF = 2047
    // target = 0x1000 + 4 + 2047*2 = 0x1000 + 4 + 4094 = 0x2002
    assert_eq!(inst.branch_target(0x1000), Some(0x2002));
}

#[test]
fn test_branch_target_bra_large_negative() {
    let inst = decode(0xA800); // BRA disp=0x800 -> sign_ext = -2048
    // target = 0x2000 + 4 + (-2048)*2 = 0x2000 + 4 - 4096 = 0x1004
    assert_eq!(inst.branch_target(0x2000), Some(0x1004));
}

#[test]
fn test_branch_target_bsr() {
    let inst = decode(0xB100); // BSR disp=0x100 = 256
    // target = 0x06004000 + 4 + 256*2 = 0x06004204
    assert_eq!(inst.branch_target(0x06004000), Some(0x06004204));
}

#[test]
fn test_decode_bsrf() {
    // BSRF R3: encoding = 0000nnnn00000011 = 0x0303 (n=3, m=0, d4=3)
    let inst = decode(0x0303);
    assert!(matches!(inst.kind, InstructionKind::Bsrf { rm } if rm == Reg(3)));
    assert_eq!(inst.flow, FlowKind::Call);
    assert!(inst.has_delay_slot());
    // Register-relative: target not statically determinable
    assert_eq!(inst.branch_target(0x1000), None);
}

#[test]
fn test_decode_braf() {
    // BRAF R5 = 0x0053 (n=5, m=0) + 0x0023 pattern → actually 0x0053
    // Wait: BRAF Rm encoding is 0000nnnn00100011 = 0x0n23
    // BRAF R5 = 0x0523
    let inst = decode(0x0523);
    assert!(matches!(inst.kind, InstructionKind::Braf { rm } if rm == Reg(5)));
    assert_eq!(inst.flow, FlowKind::UnconditionalBranch);
    assert!(inst.has_delay_slot());
    assert_eq!(inst.branch_target(0x2000), None);
}

#[test]
fn test_decode_bsrf_r0() {
    // BSRF R0 = 0x0003
    let inst = decode(0x0003);
    assert!(matches!(inst.kind, InstructionKind::Bsrf { rm } if rm == Reg(0)));
}

#[test]
fn test_decode_tas() {
    // TAS.B @R4 = 0x441B (0100nnnn00011011, n=4)
    let inst = decode(0x441B);
    assert!(matches!(inst.kind, InstructionKind::Tas { rn } if rn == Reg(4)));
    assert_eq!(inst.flow, FlowKind::Normal);
    assert!(!inst.has_delay_slot());
}
