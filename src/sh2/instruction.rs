/// SH-2 general-purpose register (R0-R15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reg(pub u8);

impl Reg {
    pub const R0: Reg = Reg(0);
    pub const SP: Reg = Reg(15);
}

impl std::fmt::Display for Reg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "R{}", self.0)
    }
}

/// How an instruction affects control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    /// Normal instruction -- falls through to the next.
    Normal,
    /// Unconditional branch (BRA, JMP) -- no fall-through, has delay slot.
    UnconditionalBranch,
    /// Conditional branch (BT, BF) -- fall-through + branch target, NO delay slot.
    ConditionalBranch,
    /// Conditional branch with delay slot (BT/S, BF/S).
    ConditionalBranchDelayed,
    /// Subroutine call (BSR, JSR) -- fall-through after delay slot + call target.
    Call,
    /// Subroutine return (RTS) -- has delay slot.
    Return,
    /// Exception return (RTE) -- has delay slot.
    ExceptionReturn,
}

/// A decoded SH-2 instruction.
#[derive(Debug, Clone)]
pub struct Instruction {
    /// The raw 16-bit opcode.
    pub opcode: u16,
    /// Instruction mnemonic and operand information.
    pub kind: InstructionKind,
    /// Control-flow classification.
    pub flow: FlowKind,
}

/// Instruction kind enum covering the full SH-2 instruction set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionKind {
    // === Data transfer ===
    /// MOV #imm, Rn (Enii -- 8-bit sign-extended immediate)
    MovImm { imm: i8, rn: Reg },
    /// MOV.W @(disp,PC), Rn (9ndd)
    MovWPcRel { disp: u8, rn: Reg },
    /// MOV.L @(disp,PC), Rn (Dndd)
    MovLPcRel { disp: u8, rn: Reg },
    /// MOV Rm, Rn (6nm3)
    MovReg { rm: Reg, rn: Reg },
    /// MOV.B Rm, @Rn (2nm0)
    MovBStore { rm: Reg, rn: Reg },
    /// MOV.W Rm, @Rn (2nm1)
    MovWStore { rm: Reg, rn: Reg },
    /// MOV.L Rm, @Rn (2nm2)
    MovLStore { rm: Reg, rn: Reg },
    /// MOV.B @Rm, Rn (6nm0)
    MovBLoad { rm: Reg, rn: Reg },
    /// MOV.W @Rm, Rn (6nm1)
    MovWLoad { rm: Reg, rn: Reg },
    /// MOV.L @Rm, Rn (6nm2)
    MovLLoad { rm: Reg, rn: Reg },
    /// MOV.B Rm, @-Rn (2nm4)
    MovBStorePreDec { rm: Reg, rn: Reg },
    /// MOV.W Rm, @-Rn (2nm5)
    MovWStorePreDec { rm: Reg, rn: Reg },
    /// MOV.L Rm, @-Rn (2nm6)
    MovLStorePreDec { rm: Reg, rn: Reg },
    /// MOV.B @Rm+, Rn (6nm4)
    MovBLoadPostInc { rm: Reg, rn: Reg },
    /// MOV.W @Rm+, Rn (6nm5)
    MovWLoadPostInc { rm: Reg, rn: Reg },
    /// MOV.L @Rm+, Rn (6nm6)
    MovLLoadPostInc { rm: Reg, rn: Reg },
    /// MOV.B @(R0,Rm), Rn (0nmC)
    MovBR0LoadIndexed { rm: Reg, rn: Reg },
    /// MOV.W @(R0,Rm), Rn (0nmD)
    MovWR0LoadIndexed { rm: Reg, rn: Reg },
    /// MOV.L @(R0,Rm), Rn (0nmE)
    MovLR0LoadIndexed { rm: Reg, rn: Reg },
    /// MOV.B Rm, @(R0,Rn) (0nm4)
    MovBR0StoreIndexed { rm: Reg, rn: Reg },
    /// MOV.W Rm, @(R0,Rn) (0nm5)
    MovWR0StoreIndexed { rm: Reg, rn: Reg },
    /// MOV.L Rm, @(R0,Rn) (0nm6)
    MovLR0StoreIndexed { rm: Reg, rn: Reg },
    /// MOV.B R0, @(disp,Rn) (80nd)
    MovBDispStore { disp: u8, rn: Reg },
    /// MOV.W R0, @(disp,Rn) (81nd)
    MovWDispStore { disp: u8, rn: Reg },
    /// MOV.L Rm, @(disp,Rn) (1nmd)
    MovLDispStore { disp: u8, rm: Reg, rn: Reg },
    /// MOV.B @(disp,Rm), R0 (84md)
    MovBDispLoad { disp: u8, rm: Reg },
    /// MOV.W @(disp,Rm), R0 (85md)
    MovWDispLoad { disp: u8, rm: Reg },
    /// MOV.L @(disp,Rm), Rn (5nmd)
    MovLDispLoad { disp: u8, rm: Reg, rn: Reg },
    /// MOV.B @(disp,GBR), R0 (C4dd) / MOV.B R0, @(disp,GBR) (C0dd)
    MovBGbr { disp: u8, store: bool },
    /// MOV.W @(disp,GBR), R0 (C5dd) / MOV.W R0, @(disp,GBR) (C1dd)
    MovWGbr { disp: u8, store: bool },
    /// MOV.L @(disp,GBR), R0 (C6dd) / MOV.L R0, @(disp,GBR) (C2dd)
    MovLGbr { disp: u8, store: bool },
    /// MOVA @(disp,PC), R0 (C7dd)
    Mova { disp: u8 },
    /// MOVT Rn (0n29)
    MovT { rn: Reg },

    // === Arithmetic ===
    /// ADD Rm, Rn (3nmc)
    Add { rm: Reg, rn: Reg },
    /// ADD #imm, Rn (7nii)
    AddImm { imm: i8, rn: Reg },
    /// ADDC Rm, Rn (3nme)
    Addc { rm: Reg, rn: Reg },
    /// ADDV Rm, Rn (3nmf)
    Addv { rm: Reg, rn: Reg },
    /// SUB Rm, Rn (3nm8)
    Sub { rm: Reg, rn: Reg },
    /// SUBC Rm, Rn (3nma)
    Subc { rm: Reg, rn: Reg },
    /// SUBV Rm, Rn (3nmb)
    Subv { rm: Reg, rn: Reg },
    /// NEG Rm, Rn (6nmb)
    Neg { rm: Reg, rn: Reg },
    /// NEGC Rm, Rn (6nma)
    Negc { rm: Reg, rn: Reg },
    /// DT Rn (4n10)
    Dt { rn: Reg },
    /// MUL.L Rm, Rn (0nm7)
    MulL { rm: Reg, rn: Reg },
    /// MULS.W Rm, Rn (2nmf)
    MulS { rm: Reg, rn: Reg },
    /// MULU.W Rm, Rn (2nme)
    MulU { rm: Reg, rn: Reg },
    /// DMULS.L Rm, Rn (3nmd)
    DmulS { rm: Reg, rn: Reg },
    /// DMULU.L Rm, Rn (3nm5)
    DmulU { rm: Reg, rn: Reg },
    /// DIV0S Rm, Rn (2nm7)
    Div0S { rm: Reg, rn: Reg },
    /// DIV0U (0019)
    Div0U,
    /// DIV1 Rm, Rn (3nm4)
    Div1 { rm: Reg, rn: Reg },
    /// MAC.L @Rm+, @Rn+ (0nmf)
    MacL { rm: Reg, rn: Reg },
    /// MAC.W @Rm+, @Rn+ (4nmf)
    MacW { rm: Reg, rn: Reg },

    // === Logical ===
    /// AND Rm, Rn (2nm9)
    And { rm: Reg, rn: Reg },
    /// AND #imm, R0 (C9ii)
    AndImm { imm: u8 },
    /// AND.B #imm, @(R0,GBR) (CDii)
    AndB { imm: u8 },
    /// OR Rm, Rn (2nmb)
    Or { rm: Reg, rn: Reg },
    /// OR #imm, R0 (CBii)
    OrImm { imm: u8 },
    /// OR.B #imm, @(R0,GBR) (CFii)
    OrB { imm: u8 },
    /// XOR Rm, Rn (2nma)
    Xor { rm: Reg, rn: Reg },
    /// XOR #imm, R0 (CAii)
    XorImm { imm: u8 },
    /// XOR.B #imm, @(R0,GBR) (CEii)
    XorB { imm: u8 },
    /// NOT Rm, Rn (6nm7)
    Not { rm: Reg, rn: Reg },
    /// TST Rm, Rn (2nm8)
    Tst { rm: Reg, rn: Reg },
    /// TST #imm, R0 (C8ii)
    TstImm { imm: u8 },
    /// TST.B #imm, @(R0,GBR) (CCii)
    TstB { imm: u8 },

    // === Shifts ===
    /// SHLL Rn (4n00)
    Shll { rn: Reg },
    /// SHLR Rn (4n01)
    Shlr { rn: Reg },
    /// SHAL Rn (4n20)
    Shal { rn: Reg },
    /// SHAR Rn (4n21)
    Shar { rn: Reg },
    /// SHLL2 Rn (4n08)
    Shll2 { rn: Reg },
    /// SHLR2 Rn (4n09)
    Shlr2 { rn: Reg },
    /// SHLL8 Rn (4n18)
    Shll8 { rn: Reg },
    /// SHLR8 Rn (4n19)
    Shlr8 { rn: Reg },
    /// SHLL16 Rn (4n28)
    Shll16 { rn: Reg },
    /// SHLR16 Rn (4n29)
    Shlr16 { rn: Reg },
    /// ROTL Rn (4n04)
    Rotl { rn: Reg },
    /// ROTR Rn (4n05)
    Rotr { rn: Reg },
    /// ROTCL Rn (4n24)
    Rotcl { rn: Reg },
    /// ROTCR Rn (4n25)
    Rotcr { rn: Reg },

    // === Sign/zero extension ===
    /// EXTS.B Rm, Rn (6nme)
    ExtsB { rm: Reg, rn: Reg },
    /// EXTS.W Rm, Rn (6nmf)
    ExtsW { rm: Reg, rn: Reg },
    /// EXTU.B Rm, Rn (6nmc)
    ExtuB { rm: Reg, rn: Reg },
    /// EXTU.W Rm, Rn (6nmd)
    ExtuW { rm: Reg, rn: Reg },
    /// SWAP.B Rm, Rn (6nm8)
    SwapB { rm: Reg, rn: Reg },
    /// SWAP.W Rm, Rn (6nm9)
    SwapW { rm: Reg, rn: Reg },
    /// XTRCT Rm, Rn (2nmd)
    Xtrct { rm: Reg, rn: Reg },

    // === Comparisons (set T-bit) ===
    /// CMP/EQ Rm, Rn (3nm0)
    CmpEq { rm: Reg, rn: Reg },
    /// CMP/EQ #imm, R0 (88ii)
    CmpEqImm { imm: i8 },
    /// CMP/GE Rm, Rn (3nm3)
    CmpGe { rm: Reg, rn: Reg },
    /// CMP/GT Rm, Rn (3nm7)
    CmpGt { rm: Reg, rn: Reg },
    /// CMP/HI Rm, Rn (3nm6)
    CmpHi { rm: Reg, rn: Reg },
    /// CMP/HS Rm, Rn (3nm2)
    CmpHs { rm: Reg, rn: Reg },
    /// CMP/PL Rn (4n15)
    CmpPl { rn: Reg },
    /// CMP/PZ Rn (4n11)
    CmpPz { rn: Reg },
    /// CMP/STR Rm, Rn (2nmc)
    CmpStr { rm: Reg, rn: Reg },

    // === Branches ===
    /// BT disp (89dd) -- no delay slot
    Bt { disp: i8 },
    /// BF disp (8Bdd) -- no delay slot
    Bf { disp: i8 },
    /// BT/S disp (8Ddd) -- delay slot
    Bts { disp: i8 },
    /// BF/S disp (8Fdd) -- delay slot
    Bfs { disp: i8 },
    /// BRA disp (Addd) -- 12-bit signed
    Bra { disp: i16 },
    /// BSR disp (Bddd) -- 12-bit signed
    Bsr { disp: i16 },
    /// JMP @Rm (4n2b)
    Jmp { rm: Reg },
    /// JSR @Rm (4n0b)
    Jsr { rm: Reg },
    /// RTS (000B)
    Rts,
    /// RTE (002B)
    Rte,

    // === System register transfer ===
    /// LDC Rm, GBR (4n1e)
    LdcGbr { rm: Reg },
    /// LDC Rm, VBR (4n2e)
    LdcVbr { rm: Reg },
    /// LDC Rm, SR (4n0e)
    LdcSr { rm: Reg },
    /// LDC.L @Rm+, GBR (4n17)
    LdcLGbr { rm: Reg },
    /// LDC.L @Rm+, VBR (4n27)
    LdcLVbr { rm: Reg },
    /// LDC.L @Rm+, SR (4n07)
    LdcLSr { rm: Reg },
    /// STC GBR, Rn (0n12)
    StcGbr { rn: Reg },
    /// STC VBR, Rn (0n22)
    StcVbr { rn: Reg },
    /// STC SR, Rn (0n02)
    StcSr { rn: Reg },
    /// STC.L GBR, @-Rn (4n13)
    StcLGbr { rn: Reg },
    /// STC.L VBR, @-Rn (4n23)
    StcLVbr { rn: Reg },
    /// STC.L SR, @-Rn (4n03)
    StcLSr { rn: Reg },
    /// LDS Rm, MACH (4n0a)
    LdsMach { rm: Reg },
    /// LDS Rm, MACL (4n1a)
    LdsMacl { rm: Reg },
    /// LDS Rm, PR (4n2a)
    LdsPr { rm: Reg },
    /// LDS.L @Rm+, MACH (4n06)
    LdsLMach { rm: Reg },
    /// LDS.L @Rm+, MACL (4n16)
    LdsLMacl { rm: Reg },
    /// LDS.L @Rm+, PR (4n26)
    LdsLPr { rm: Reg },
    /// STS MACH, Rn (0n0a)
    StsMach { rn: Reg },
    /// STS MACL, Rn (0n1a)
    StsMacl { rn: Reg },
    /// STS PR, Rn (0n2a)
    StsPr { rn: Reg },
    /// STS.L MACH, @-Rn (4n02)
    StsLMach { rn: Reg },
    /// STS.L MACL, @-Rn (4n12)
    StsLMacl { rn: Reg },
    /// STS.L PR, @-Rn (4n22)
    StsLPr { rn: Reg },

    // === Special ===
    /// NOP (0009)
    Nop,
    /// SLEEP (001B)
    Sleep,
    /// SETT (0018)
    SetT,
    /// CLRT (0008)
    ClrT,
    /// CLRMAC (0028)
    ClrMac,
    /// TRAPA #imm (C3ii)
    Trapa { imm: u8 },
    /// BSRF Rm (0n03) -- Branch to Subroutine Far: PC + 4 + Rm → PC, PC + 4 → PR
    Bsrf { rm: Reg },
    /// BRAF Rm (0n23) -- Branch Far: PC + 4 + Rm → PC
    Braf { rm: Reg },
    /// TAS.B @Rn (4n1B) -- Test And Set byte at @Rn
    Tas { rn: Reg },

    /// Undecodable opcode.
    Unknown { opcode: u16 },
}

impl Instruction {
    /// Whether this instruction has a delay slot.
    pub fn has_delay_slot(&self) -> bool {
        matches!(
            self.flow,
            FlowKind::UnconditionalBranch
                | FlowKind::ConditionalBranchDelayed
                | FlowKind::Call
                | FlowKind::Return
                | FlowKind::ExceptionReturn
        )
    }

    /// Compute the PC-relative branch target address.
    ///
    /// `pc` is the address of this instruction itself.
    /// Returns `None` for non-branch instructions or register-indirect branches.
    pub fn branch_target(&self, pc: u32) -> Option<u32> {
        match &self.kind {
            InstructionKind::Bt { disp }
            | InstructionKind::Bf { disp }
            | InstructionKind::Bts { disp }
            | InstructionKind::Bfs { disp } => {
                // target = PC + 4 + sign_extended(disp) * 2
                Some((pc as i64 + 4 + (*disp as i64) * 2) as u32)
            }
            InstructionKind::Bra { disp } | InstructionKind::Bsr { disp } => {
                // target = PC + 4 + sign_extended(disp) * 2  (12-bit signed)
                Some((pc as i64 + 4 + (*disp as i64) * 2) as u32)
            }
            _ => None,
        }
    }

    /// Compute the literal pool address for PC-relative load instructions.
    ///
    /// `pc` is the address of this instruction itself.
    pub fn literal_pool_addr(&self, pc: u32) -> Option<u32> {
        match &self.kind {
            InstructionKind::MovLPcRel { disp, .. } => {
                // addr = (PC & 0xFFFFFFFC) + 4 + disp * 4
                Some((pc & 0xFFFF_FFFC) + 4 + (*disp as u32) * 4)
            }
            InstructionKind::MovWPcRel { disp, .. } => {
                // addr = PC + 4 + disp * 2  (no alignment)
                Some(pc + 4 + (*disp as u32) * 2)
            }
            InstructionKind::Mova { disp } => {
                // addr = (PC & 0xFFFFFFFC) + 4 + disp * 4
                Some((pc & 0xFFFF_FFFC) + 4 + (*disp as u32) * 4)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "instruction_tests.rs"]
mod tests;
