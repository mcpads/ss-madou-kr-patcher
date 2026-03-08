use super::instruction::{FlowKind, Instruction, InstructionKind, Reg};

/// Sign-extend an 8-bit value to i8.
fn sign_ext8(val: u8) -> i8 {
    val as i8
}

/// Sign-extend a 12-bit value to i16.
fn sign_ext12(val: u16) -> i16 {
    if val & 0x800 != 0 {
        (val | 0xF000) as i16
    } else {
        val as i16
    }
}

/// Helper to construct an Instruction with a given kind and flow.
fn inst(opcode: u16, kind: InstructionKind, flow: FlowKind) -> Instruction {
    Instruction { opcode, kind, flow }
}

/// Helper to construct a Normal-flow instruction.
fn normal(opcode: u16, kind: InstructionKind) -> Instruction {
    inst(opcode, kind, FlowKind::Normal)
}

/// Helper to construct an Unknown instruction.
fn unknown(opcode: u16) -> Instruction {
    normal(opcode, InstructionKind::Unknown { opcode })
}

/// Decode a 16-bit big-endian SH-2 opcode into an `Instruction`.
pub fn decode(opcode: u16) -> Instruction {
    let n = ((opcode >> 8) & 0x0F) as u8; // Rn field (bits 8-11)
    let m = ((opcode >> 4) & 0x0F) as u8; // Rm field (bits 4-7)
    let d4 = (opcode & 0x0F) as u8; // low 4 bits
    let d8 = (opcode & 0xFF) as u8; // low 8 bits
    let d12 = opcode & 0x0FFF; // low 12 bits

    match opcode >> 12 {
        0x0 => decode_0xxx(opcode, n, m, d4),
        0x1 => {
            // MOV.L Rm, @(disp,Rn) -- 1nmd
            normal(
                opcode,
                InstructionKind::MovLDispStore {
                    disp: d4,
                    rm: Reg(m),
                    rn: Reg(n),
                },
            )
        }
        0x2 => decode_2xxx(opcode, n, m, d4),
        0x3 => decode_3xxx(opcode, n, m, d4),
        0x4 => decode_4xxx(opcode, n, m, d4),
        0x5 => {
            // MOV.L @(disp,Rm), Rn -- 5nmd
            normal(
                opcode,
                InstructionKind::MovLDispLoad {
                    disp: d4,
                    rm: Reg(m),
                    rn: Reg(n),
                },
            )
        }
        0x6 => decode_6xxx(opcode, n, m, d4),
        0x7 => {
            // ADD #imm, Rn -- 7nii
            normal(
                opcode,
                InstructionKind::AddImm {
                    imm: sign_ext8(d8),
                    rn: Reg(n),
                },
            )
        }
        0x8 => decode_8xxx(opcode, n, d8),
        0x9 => {
            // MOV.W @(disp,PC), Rn -- 9ndd
            normal(
                opcode,
                InstructionKind::MovWPcRel {
                    disp: d8,
                    rn: Reg(n),
                },
            )
        }
        0xA => {
            // BRA disp -- Addd (12-bit signed)
            inst(
                opcode,
                InstructionKind::Bra {
                    disp: sign_ext12(d12),
                },
                FlowKind::UnconditionalBranch,
            )
        }
        0xB => {
            // BSR disp -- Bddd (12-bit signed)
            inst(
                opcode,
                InstructionKind::Bsr {
                    disp: sign_ext12(d12),
                },
                FlowKind::Call,
            )
        }
        0xC => decode_cxxx(opcode, d8),
        0xD => {
            // MOV.L @(disp,PC), Rn -- Dndd
            normal(
                opcode,
                InstructionKind::MovLPcRel {
                    disp: d8,
                    rn: Reg(n),
                },
            )
        }
        0xE => {
            // MOV #imm, Rn -- Enii (8-bit sign-extended)
            normal(
                opcode,
                InstructionKind::MovImm {
                    imm: sign_ext8(d8),
                    rn: Reg(n),
                },
            )
        }
        0xF => {
            // Undefined on SH-2 (SH-4 FPU instructions)
            unknown(opcode)
        }
        _ => unreachable!(),
    }
}

/// Decode 0xxx instructions: system, STC, STS, MUL.L, MAC.L, MOV R0-indexed, etc.
fn decode_0xxx(opcode: u16, n: u8, m: u8, d4: u8) -> Instruction {
    match d4 {
        0x2 => {
            // 0n02: STC SR, Rn
            // 0n12: STC GBR, Rn
            // 0n22: STC VBR, Rn
            match m & 0x03 {
                0 => normal(opcode, InstructionKind::StcSr { rn: Reg(n) }),
                1 => normal(opcode, InstructionKind::StcGbr { rn: Reg(n) }),
                2 => normal(opcode, InstructionKind::StcVbr { rn: Reg(n) }),
                _ => unknown(opcode),
            }
        }
        0x3 => {
            // 0nm3: BSRF/BRAF -- standard SH-2 instructions (register-relative branch)
            // Target = PC + 4 + Rm, resolved at runtime (not statically determinable)
            match m {
                0 => inst(opcode, InstructionKind::Bsrf { rm: Reg(n) }, FlowKind::Call),
                2 => inst(
                    opcode,
                    InstructionKind::Braf { rm: Reg(n) },
                    FlowKind::UnconditionalBranch,
                ),
                _ => unknown(opcode),
            }
        }
        0x4 => {
            // 0nm4: MOV.B Rm, @(R0,Rn)
            normal(
                opcode,
                InstructionKind::MovBR0StoreIndexed {
                    rm: Reg(m),
                    rn: Reg(n),
                },
            )
        }
        0x5 => {
            // 0nm5: MOV.W Rm, @(R0,Rn)
            normal(
                opcode,
                InstructionKind::MovWR0StoreIndexed {
                    rm: Reg(m),
                    rn: Reg(n),
                },
            )
        }
        0x6 => {
            // 0nm6: MOV.L Rm, @(R0,Rn)
            normal(
                opcode,
                InstructionKind::MovLR0StoreIndexed {
                    rm: Reg(m),
                    rn: Reg(n),
                },
            )
        }
        0x7 => {
            // 0nm7: MUL.L Rm, Rn
            normal(
                opcode,
                InstructionKind::MulL {
                    rm: Reg(m),
                    rn: Reg(n),
                },
            )
        }
        0x8 => {
            // 0x08: CLRT
            // 0x18: SETT
            // 0x28: CLRMAC
            // 0x38+: unknown
            match opcode {
                0x0008 => normal(opcode, InstructionKind::ClrT),
                0x0018 => normal(opcode, InstructionKind::SetT),
                0x0028 => normal(opcode, InstructionKind::ClrMac),
                _ => unknown(opcode),
            }
        }
        0x9 => {
            // 0x09: NOP
            // 0x19: DIV0U
            // 0x29: MOVT Rn -- but actually MOVT is 0n29 format
            match opcode {
                0x0009 => normal(opcode, InstructionKind::Nop),
                0x0019 => normal(opcode, InstructionKind::Div0U),
                _ => {
                    // 0n29: MOVT Rn (where n != 0 potentially, but encoding is 0n29)
                    if m == 2 {
                        normal(opcode, InstructionKind::MovT { rn: Reg(n) })
                    } else {
                        unknown(opcode)
                    }
                }
            }
        }
        0xA => {
            // 0n0a: STS MACH, Rn
            // 0n1a: STS MACL, Rn
            // 0n2a: STS PR, Rn
            match m & 0x03 {
                0 => normal(opcode, InstructionKind::StsMach { rn: Reg(n) }),
                1 => normal(opcode, InstructionKind::StsMacl { rn: Reg(n) }),
                2 => normal(opcode, InstructionKind::StsPr { rn: Reg(n) }),
                _ => unknown(opcode),
            }
        }
        0xB => {
            // 0x0B: RTS
            // 0x1B: SLEEP
            // 0x2B: RTE
            match opcode {
                0x000B => inst(opcode, InstructionKind::Rts, FlowKind::Return),
                0x001B => normal(opcode, InstructionKind::Sleep),
                0x002B => inst(opcode, InstructionKind::Rte, FlowKind::ExceptionReturn),
                _ => unknown(opcode),
            }
        }
        0xC => {
            // 0nmC: MOV.B @(R0,Rm), Rn
            normal(
                opcode,
                InstructionKind::MovBR0LoadIndexed {
                    rm: Reg(m),
                    rn: Reg(n),
                },
            )
        }
        0xD => {
            // 0nmD: MOV.W @(R0,Rm), Rn
            normal(
                opcode,
                InstructionKind::MovWR0LoadIndexed {
                    rm: Reg(m),
                    rn: Reg(n),
                },
            )
        }
        0xE => {
            // 0nmE: MOV.L @(R0,Rm), Rn
            normal(
                opcode,
                InstructionKind::MovLR0LoadIndexed {
                    rm: Reg(m),
                    rn: Reg(n),
                },
            )
        }
        0xF => {
            // 0nmF: MAC.L @Rm+, @Rn+
            normal(
                opcode,
                InstructionKind::MacL {
                    rm: Reg(m),
                    rn: Reg(n),
                },
            )
        }
        _ => {
            // 0x0, 0x1: unknown encodings in this group
            unknown(opcode)
        }
    }
}

/// Decode 2xxx instructions: MOV stores, pre-decrement, logical, etc.
fn decode_2xxx(opcode: u16, n: u8, m: u8, d4: u8) -> Instruction {
    match d4 {
        0x0 => normal(
            opcode,
            InstructionKind::MovBStore {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x1 => normal(
            opcode,
            InstructionKind::MovWStore {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x2 => normal(
            opcode,
            InstructionKind::MovLStore {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x4 => normal(
            opcode,
            InstructionKind::MovBStorePreDec {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x5 => normal(
            opcode,
            InstructionKind::MovWStorePreDec {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x6 => normal(
            opcode,
            InstructionKind::MovLStorePreDec {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x7 => normal(
            opcode,
            InstructionKind::Div0S {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x8 => normal(
            opcode,
            InstructionKind::Tst {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x9 => normal(
            opcode,
            InstructionKind::And {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xA => normal(
            opcode,
            InstructionKind::Xor {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xB => normal(
            opcode,
            InstructionKind::Or {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xC => normal(
            opcode,
            InstructionKind::CmpStr {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xD => normal(
            opcode,
            InstructionKind::Xtrct {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xE => normal(
            opcode,
            InstructionKind::MulU {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xF => normal(
            opcode,
            InstructionKind::MulS {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        _ => {
            // 0x3: undefined in 2xxx group
            unknown(opcode)
        }
    }
}

/// Decode 3xxx instructions: arithmetic, compare, div, dmul.
fn decode_3xxx(opcode: u16, n: u8, m: u8, d4: u8) -> Instruction {
    match d4 {
        0x0 => normal(
            opcode,
            InstructionKind::CmpEq {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x2 => normal(
            opcode,
            InstructionKind::CmpHs {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x3 => normal(
            opcode,
            InstructionKind::CmpGe {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x4 => normal(
            opcode,
            InstructionKind::Div1 {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x5 => normal(
            opcode,
            InstructionKind::DmulU {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x6 => normal(
            opcode,
            InstructionKind::CmpHi {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x7 => normal(
            opcode,
            InstructionKind::CmpGt {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x8 => normal(
            opcode,
            InstructionKind::Sub {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xA => normal(
            opcode,
            InstructionKind::Subc {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xB => normal(
            opcode,
            InstructionKind::Subv {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xC => normal(
            opcode,
            InstructionKind::Add {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xD => normal(
            opcode,
            InstructionKind::DmulS {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xE => normal(
            opcode,
            InstructionKind::Addc {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xF => normal(
            opcode,
            InstructionKind::Addv {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        _ => {
            // 0x1, 0x9: undefined in 3xxx group
            unknown(opcode)
        }
    }
}

/// Decode 4xxx instructions: shifts, rotates, JSR, JMP, DT, CMP/Px, system register ops.
fn decode_4xxx(opcode: u16, n: u8, m: u8, d4: u8) -> Instruction {
    // 4xxx is complex: the low byte (m:d4) determines the instruction.
    let low_byte = ((m as u16) << 4) | d4 as u16;

    match low_byte as u8 {
        0x00 => normal(opcode, InstructionKind::Shll { rn: Reg(n) }),
        0x01 => normal(opcode, InstructionKind::Shlr { rn: Reg(n) }),
        0x02 => normal(opcode, InstructionKind::StsLMach { rn: Reg(n) }),
        0x03 => normal(opcode, InstructionKind::StcLSr { rn: Reg(n) }),
        0x04 => normal(opcode, InstructionKind::Rotl { rn: Reg(n) }),
        0x05 => normal(opcode, InstructionKind::Rotr { rn: Reg(n) }),
        0x06 => normal(opcode, InstructionKind::LdsLMach { rm: Reg(n) }),
        0x07 => normal(opcode, InstructionKind::LdcLSr { rm: Reg(n) }),
        0x08 => normal(opcode, InstructionKind::Shll2 { rn: Reg(n) }),
        0x09 => normal(opcode, InstructionKind::Shlr2 { rn: Reg(n) }),
        0x0A => normal(opcode, InstructionKind::LdsMach { rm: Reg(n) }),
        0x0B => inst(opcode, InstructionKind::Jsr { rm: Reg(n) }, FlowKind::Call),
        0x0E => normal(opcode, InstructionKind::LdcSr { rm: Reg(n) }),
        0x10 => normal(opcode, InstructionKind::Dt { rn: Reg(n) }),
        0x11 => normal(opcode, InstructionKind::CmpPz { rn: Reg(n) }),
        0x12 => normal(opcode, InstructionKind::StsLMacl { rn: Reg(n) }),
        0x13 => normal(opcode, InstructionKind::StcLGbr { rn: Reg(n) }),
        0x15 => normal(opcode, InstructionKind::CmpPl { rn: Reg(n) }),
        0x16 => normal(opcode, InstructionKind::LdsLMacl { rm: Reg(n) }),
        0x17 => normal(opcode, InstructionKind::LdcLGbr { rm: Reg(n) }),
        0x18 => normal(opcode, InstructionKind::Shll8 { rn: Reg(n) }),
        0x19 => normal(opcode, InstructionKind::Shlr8 { rn: Reg(n) }),
        0x1A => normal(opcode, InstructionKind::LdsMacl { rm: Reg(n) }),
        0x1B => normal(opcode, InstructionKind::Tas { rn: Reg(n) }),
        0x1E => normal(opcode, InstructionKind::LdcGbr { rm: Reg(n) }),
        0x20 => normal(opcode, InstructionKind::Shal { rn: Reg(n) }),
        0x21 => normal(opcode, InstructionKind::Shar { rn: Reg(n) }),
        0x22 => normal(opcode, InstructionKind::StsLPr { rn: Reg(n) }),
        0x23 => normal(opcode, InstructionKind::StcLVbr { rn: Reg(n) }),
        0x24 => normal(opcode, InstructionKind::Rotcl { rn: Reg(n) }),
        0x25 => normal(opcode, InstructionKind::Rotcr { rn: Reg(n) }),
        0x26 => normal(opcode, InstructionKind::LdsLPr { rm: Reg(n) }),
        0x27 => normal(opcode, InstructionKind::LdcLVbr { rm: Reg(n) }),
        0x28 => normal(opcode, InstructionKind::Shll16 { rn: Reg(n) }),
        0x29 => normal(opcode, InstructionKind::Shlr16 { rn: Reg(n) }),
        0x2A => normal(opcode, InstructionKind::LdsPr { rm: Reg(n) }),
        0x2B => inst(
            opcode,
            InstructionKind::Jmp { rm: Reg(n) },
            FlowKind::UnconditionalBranch,
        ),
        0x2E => normal(opcode, InstructionKind::LdcVbr { rm: Reg(n) }),
        _ => {
            // Check for MAC.W @Rm+, @Rn+ (4nmF pattern)
            if d4 == 0xF {
                normal(
                    opcode,
                    InstructionKind::MacW {
                        rm: Reg(m),
                        rn: Reg(n),
                    },
                )
            } else {
                unknown(opcode)
            }
        }
    }
}

/// Decode 6xxx instructions: MOV loads, post-increment, extensions, swaps, etc.
fn decode_6xxx(opcode: u16, n: u8, m: u8, d4: u8) -> Instruction {
    match d4 {
        0x0 => normal(
            opcode,
            InstructionKind::MovBLoad {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x1 => normal(
            opcode,
            InstructionKind::MovWLoad {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x2 => normal(
            opcode,
            InstructionKind::MovLLoad {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x3 => normal(
            opcode,
            InstructionKind::MovReg {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x4 => normal(
            opcode,
            InstructionKind::MovBLoadPostInc {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x5 => normal(
            opcode,
            InstructionKind::MovWLoadPostInc {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x6 => normal(
            opcode,
            InstructionKind::MovLLoadPostInc {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x7 => normal(
            opcode,
            InstructionKind::Not {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x8 => normal(
            opcode,
            InstructionKind::SwapB {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0x9 => normal(
            opcode,
            InstructionKind::SwapW {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xA => normal(
            opcode,
            InstructionKind::Negc {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xB => normal(
            opcode,
            InstructionKind::Neg {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xC => normal(
            opcode,
            InstructionKind::ExtuB {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xD => normal(
            opcode,
            InstructionKind::ExtuW {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xE => normal(
            opcode,
            InstructionKind::ExtsB {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        0xF => normal(
            opcode,
            InstructionKind::ExtsW {
                rm: Reg(m),
                rn: Reg(n),
            },
        ),
        _ => unreachable!(),
    }
}

/// Decode 8xxx instructions: BT, BF, BT/S, BF/S, MOV.B disp, MOV.W disp, CMP/EQ #imm.
fn decode_8xxx(opcode: u16, n: u8, d8: u8) -> Instruction {
    // In 8xxx group, bits 8-11 determine the sub-instruction.
    match n {
        0x0 => {
            // 80md: MOV.B R0, @(disp,Rm) -- here "n" in 80nd means Rm, d8 has disp
            // Actually encoding is 80nd where n=Rn and d=disp (4-bit each from d8)
            let rn = (d8 >> 4) & 0x0F;
            let disp = d8 & 0x0F;
            normal(
                opcode,
                InstructionKind::MovBDispStore {
                    disp,
                    rn: Reg(rn),
                },
            )
        }
        0x1 => {
            // 81nd: MOV.W R0, @(disp,Rn)
            let rn = (d8 >> 4) & 0x0F;
            let disp = d8 & 0x0F;
            normal(
                opcode,
                InstructionKind::MovWDispStore {
                    disp,
                    rn: Reg(rn),
                },
            )
        }
        0x4 => {
            // 84md: MOV.B @(disp,Rm), R0
            let rm = (d8 >> 4) & 0x0F;
            let disp = d8 & 0x0F;
            normal(
                opcode,
                InstructionKind::MovBDispLoad {
                    disp,
                    rm: Reg(rm),
                },
            )
        }
        0x5 => {
            // 85md: MOV.W @(disp,Rm), R0
            let rm = (d8 >> 4) & 0x0F;
            let disp = d8 & 0x0F;
            normal(
                opcode,
                InstructionKind::MovWDispLoad {
                    disp,
                    rm: Reg(rm),
                },
            )
        }
        0x8 => {
            // 88ii: CMP/EQ #imm, R0
            normal(
                opcode,
                InstructionKind::CmpEqImm {
                    imm: sign_ext8(d8),
                },
            )
        }
        0x9 => {
            // 89dd: BT disp (no delay slot)
            inst(
                opcode,
                InstructionKind::Bt {
                    disp: sign_ext8(d8),
                },
                FlowKind::ConditionalBranch,
            )
        }
        0xB => {
            // 8Bdd: BF disp (no delay slot)
            inst(
                opcode,
                InstructionKind::Bf {
                    disp: sign_ext8(d8),
                },
                FlowKind::ConditionalBranch,
            )
        }
        0xD => {
            // 8Ddd: BT/S disp (with delay slot)
            inst(
                opcode,
                InstructionKind::Bts {
                    disp: sign_ext8(d8),
                },
                FlowKind::ConditionalBranchDelayed,
            )
        }
        0xF => {
            // 8Fdd: BF/S disp (with delay slot)
            inst(
                opcode,
                InstructionKind::Bfs {
                    disp: sign_ext8(d8),
                },
                FlowKind::ConditionalBranchDelayed,
            )
        }
        _ => unknown(opcode),
    }
}

/// Decode Cxxx instructions: GBR-relative MOV, MOVA, TRAPA, AND/OR/XOR/TST #imm.
fn decode_cxxx(opcode: u16, d8: u8) -> Instruction {
    // Upper nibble of d8 (bits 8-11, which is actually the "n" field) determines sub-instruction
    let sub = (opcode >> 8) & 0x0F;
    match sub {
        0x0 => normal(
            opcode,
            InstructionKind::MovBGbr {
                disp: d8,
                store: true,
            },
        ),
        0x1 => normal(
            opcode,
            InstructionKind::MovWGbr {
                disp: d8,
                store: true,
            },
        ),
        0x2 => normal(
            opcode,
            InstructionKind::MovLGbr {
                disp: d8,
                store: true,
            },
        ),
        0x3 => normal(opcode, InstructionKind::Trapa { imm: d8 }),
        0x4 => normal(
            opcode,
            InstructionKind::MovBGbr {
                disp: d8,
                store: false,
            },
        ),
        0x5 => normal(
            opcode,
            InstructionKind::MovWGbr {
                disp: d8,
                store: false,
            },
        ),
        0x6 => normal(
            opcode,
            InstructionKind::MovLGbr {
                disp: d8,
                store: false,
            },
        ),
        0x7 => normal(opcode, InstructionKind::Mova { disp: d8 }),
        0x8 => normal(opcode, InstructionKind::TstImm { imm: d8 }),
        0x9 => normal(opcode, InstructionKind::AndImm { imm: d8 }),
        0xA => normal(opcode, InstructionKind::XorImm { imm: d8 }),
        0xB => normal(opcode, InstructionKind::OrImm { imm: d8 }),
        0xC => normal(opcode, InstructionKind::TstB { imm: d8 }),
        0xD => normal(opcode, InstructionKind::AndB { imm: d8 }),
        0xE => normal(opcode, InstructionKind::XorB { imm: d8 }),
        0xF => normal(opcode, InstructionKind::OrB { imm: d8 }),
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod tests;
