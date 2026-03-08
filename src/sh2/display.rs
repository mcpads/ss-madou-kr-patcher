use std::fmt;

use super::instruction::{Instruction, InstructionKind};

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            // === Data transfer ===
            InstructionKind::MovImm { imm, rn } => write!(f, "MOV     #{},{}", imm, rn),
            InstructionKind::MovWPcRel { disp, rn } => {
                write!(f, "MOV.W   @({},PC),{}", (*disp as u16) * 2, rn)
            }
            InstructionKind::MovLPcRel { disp, rn } => {
                write!(f, "MOV.L   @({},PC),{}", (*disp as u16) * 4, rn)
            }
            InstructionKind::MovReg { rm, rn } => write!(f, "MOV     {},{}", rm, rn),
            InstructionKind::MovBStore { rm, rn } => write!(f, "MOV.B   {},@{}", rm, rn),
            InstructionKind::MovWStore { rm, rn } => write!(f, "MOV.W   {},@{}", rm, rn),
            InstructionKind::MovLStore { rm, rn } => write!(f, "MOV.L   {},@{}", rm, rn),
            InstructionKind::MovBLoad { rm, rn } => write!(f, "MOV.B   @{},{}", rm, rn),
            InstructionKind::MovWLoad { rm, rn } => write!(f, "MOV.W   @{},{}", rm, rn),
            InstructionKind::MovLLoad { rm, rn } => write!(f, "MOV.L   @{},{}", rm, rn),
            InstructionKind::MovBStorePreDec { rm, rn } => {
                write!(f, "MOV.B   {},@-{}", rm, rn)
            }
            InstructionKind::MovWStorePreDec { rm, rn } => {
                write!(f, "MOV.W   {},@-{}", rm, rn)
            }
            InstructionKind::MovLStorePreDec { rm, rn } => {
                write!(f, "MOV.L   {},@-{}", rm, rn)
            }
            InstructionKind::MovBLoadPostInc { rm, rn } => {
                write!(f, "MOV.B   @{}+,{}", rm, rn)
            }
            InstructionKind::MovWLoadPostInc { rm, rn } => {
                write!(f, "MOV.W   @{}+,{}", rm, rn)
            }
            InstructionKind::MovLLoadPostInc { rm, rn } => {
                write!(f, "MOV.L   @{}+,{}", rm, rn)
            }
            InstructionKind::MovBR0LoadIndexed { rm, rn } => {
                write!(f, "MOV.B   @(R0,{}),{}", rm, rn)
            }
            InstructionKind::MovWR0LoadIndexed { rm, rn } => {
                write!(f, "MOV.W   @(R0,{}),{}", rm, rn)
            }
            InstructionKind::MovLR0LoadIndexed { rm, rn } => {
                write!(f, "MOV.L   @(R0,{}),{}", rm, rn)
            }
            InstructionKind::MovBR0StoreIndexed { rm, rn } => {
                write!(f, "MOV.B   {},@(R0,{})", rm, rn)
            }
            InstructionKind::MovWR0StoreIndexed { rm, rn } => {
                write!(f, "MOV.W   {},@(R0,{})", rm, rn)
            }
            InstructionKind::MovLR0StoreIndexed { rm, rn } => {
                write!(f, "MOV.L   {},@(R0,{})", rm, rn)
            }
            InstructionKind::MovBDispStore { disp, rn } => {
                write!(f, "MOV.B   R0,@({},{})", disp, rn)
            }
            InstructionKind::MovWDispStore { disp, rn } => {
                write!(f, "MOV.W   R0,@({},{})", (*disp as u16) * 2, rn)
            }
            InstructionKind::MovLDispStore { disp, rm, rn } => {
                write!(f, "MOV.L   {},@({},{})", rm, (*disp as u16) * 4, rn)
            }
            InstructionKind::MovBDispLoad { disp, rm } => {
                write!(f, "MOV.B   @({},{}),R0", disp, rm)
            }
            InstructionKind::MovWDispLoad { disp, rm } => {
                write!(f, "MOV.W   @({},{}),R0", (*disp as u16) * 2, rm)
            }
            InstructionKind::MovLDispLoad { disp, rm, rn } => {
                write!(f, "MOV.L   @({},{}),{}", (*disp as u16) * 4, rm, rn)
            }
            InstructionKind::MovBGbr { disp, store: true } => {
                write!(f, "MOV.B   R0,@({},GBR)", disp)
            }
            InstructionKind::MovBGbr {
                disp,
                store: false,
            } => write!(f, "MOV.B   @({},GBR),R0", disp),
            InstructionKind::MovWGbr { disp, store: true } => {
                write!(f, "MOV.W   R0,@({},GBR)", (*disp as u16) * 2)
            }
            InstructionKind::MovWGbr {
                disp,
                store: false,
            } => write!(f, "MOV.W   @({},GBR),R0", (*disp as u16) * 2),
            InstructionKind::MovLGbr { disp, store: true } => {
                write!(f, "MOV.L   R0,@({},GBR)", (*disp as u16) * 4)
            }
            InstructionKind::MovLGbr {
                disp,
                store: false,
            } => write!(f, "MOV.L   @({},GBR),R0", (*disp as u16) * 4),
            InstructionKind::Mova { disp } => {
                write!(f, "MOVA    @({},PC),R0", (*disp as u16) * 4)
            }
            InstructionKind::MovT { rn } => write!(f, "MOVT    {}", rn),

            // === Arithmetic ===
            InstructionKind::Add { rm, rn } => write!(f, "ADD     {},{}", rm, rn),
            InstructionKind::AddImm { imm, rn } => write!(f, "ADD     #{},{}", imm, rn),
            InstructionKind::Addc { rm, rn } => write!(f, "ADDC    {},{}", rm, rn),
            InstructionKind::Addv { rm, rn } => write!(f, "ADDV    {},{}", rm, rn),
            InstructionKind::Sub { rm, rn } => write!(f, "SUB     {},{}", rm, rn),
            InstructionKind::Subc { rm, rn } => write!(f, "SUBC    {},{}", rm, rn),
            InstructionKind::Subv { rm, rn } => write!(f, "SUBV    {},{}", rm, rn),
            InstructionKind::Neg { rm, rn } => write!(f, "NEG     {},{}", rm, rn),
            InstructionKind::Negc { rm, rn } => write!(f, "NEGC    {},{}", rm, rn),
            InstructionKind::Dt { rn } => write!(f, "DT      {}", rn),
            InstructionKind::MulL { rm, rn } => write!(f, "MUL.L   {},{}", rm, rn),
            InstructionKind::MulS { rm, rn } => write!(f, "MULS.W  {},{}", rm, rn),
            InstructionKind::MulU { rm, rn } => write!(f, "MULU.W  {},{}", rm, rn),
            InstructionKind::DmulS { rm, rn } => write!(f, "DMULS.L {},{}", rm, rn),
            InstructionKind::DmulU { rm, rn } => write!(f, "DMULU.L {},{}", rm, rn),
            InstructionKind::Div0S { rm, rn } => write!(f, "DIV0S   {},{}", rm, rn),
            InstructionKind::Div0U => write!(f, "DIV0U"),
            InstructionKind::Div1 { rm, rn } => write!(f, "DIV1    {},{}", rm, rn),
            InstructionKind::MacL { rm, rn } => write!(f, "MAC.L   @{}+,@{}+", rm, rn),
            InstructionKind::MacW { rm, rn } => write!(f, "MAC.W   @{}+,@{}+", rm, rn),

            // === Logical ===
            InstructionKind::And { rm, rn } => write!(f, "AND     {},{}", rm, rn),
            InstructionKind::AndImm { imm } => write!(f, "AND     #{},R0", imm),
            InstructionKind::AndB { imm } => write!(f, "AND.B   #{},@(R0,GBR)", imm),
            InstructionKind::Or { rm, rn } => write!(f, "OR      {},{}", rm, rn),
            InstructionKind::OrImm { imm } => write!(f, "OR      #{},R0", imm),
            InstructionKind::OrB { imm } => write!(f, "OR.B    #{},@(R0,GBR)", imm),
            InstructionKind::Xor { rm, rn } => write!(f, "XOR     {},{}", rm, rn),
            InstructionKind::XorImm { imm } => write!(f, "XOR     #{},R0", imm),
            InstructionKind::XorB { imm } => write!(f, "XOR.B   #{},@(R0,GBR)", imm),
            InstructionKind::Not { rm, rn } => write!(f, "NOT     {},{}", rm, rn),
            InstructionKind::Tst { rm, rn } => write!(f, "TST     {},{}", rm, rn),
            InstructionKind::TstImm { imm } => write!(f, "TST     #{},R0", imm),
            InstructionKind::TstB { imm } => write!(f, "TST.B   #{},@(R0,GBR)", imm),

            // === Shifts ===
            InstructionKind::Shll { rn } => write!(f, "SHLL    {}", rn),
            InstructionKind::Shlr { rn } => write!(f, "SHLR    {}", rn),
            InstructionKind::Shal { rn } => write!(f, "SHAL    {}", rn),
            InstructionKind::Shar { rn } => write!(f, "SHAR    {}", rn),
            InstructionKind::Shll2 { rn } => write!(f, "SHLL2   {}", rn),
            InstructionKind::Shlr2 { rn } => write!(f, "SHLR2   {}", rn),
            InstructionKind::Shll8 { rn } => write!(f, "SHLL8   {}", rn),
            InstructionKind::Shlr8 { rn } => write!(f, "SHLR8   {}", rn),
            InstructionKind::Shll16 { rn } => write!(f, "SHLL16  {}", rn),
            InstructionKind::Shlr16 { rn } => write!(f, "SHLR16  {}", rn),
            InstructionKind::Rotl { rn } => write!(f, "ROTL    {}", rn),
            InstructionKind::Rotr { rn } => write!(f, "ROTR    {}", rn),
            InstructionKind::Rotcl { rn } => write!(f, "ROTCL   {}", rn),
            InstructionKind::Rotcr { rn } => write!(f, "ROTCR   {}", rn),

            // === Sign/zero extension ===
            InstructionKind::ExtsB { rm, rn } => write!(f, "EXTS.B  {},{}", rm, rn),
            InstructionKind::ExtsW { rm, rn } => write!(f, "EXTS.W  {},{}", rm, rn),
            InstructionKind::ExtuB { rm, rn } => write!(f, "EXTU.B  {},{}", rm, rn),
            InstructionKind::ExtuW { rm, rn } => write!(f, "EXTU.W  {},{}", rm, rn),
            InstructionKind::SwapB { rm, rn } => write!(f, "SWAP.B  {},{}", rm, rn),
            InstructionKind::SwapW { rm, rn } => write!(f, "SWAP.W  {},{}", rm, rn),
            InstructionKind::Xtrct { rm, rn } => write!(f, "XTRCT   {},{}", rm, rn),

            // === Comparisons ===
            InstructionKind::CmpEq { rm, rn } => write!(f, "CMP/EQ  {},{}", rm, rn),
            InstructionKind::CmpEqImm { imm } => write!(f, "CMP/EQ  #{},R0", imm),
            InstructionKind::CmpGe { rm, rn } => write!(f, "CMP/GE  {},{}", rm, rn),
            InstructionKind::CmpGt { rm, rn } => write!(f, "CMP/GT  {},{}", rm, rn),
            InstructionKind::CmpHi { rm, rn } => write!(f, "CMP/HI  {},{}", rm, rn),
            InstructionKind::CmpHs { rm, rn } => write!(f, "CMP/HS  {},{}", rm, rn),
            InstructionKind::CmpPl { rn } => write!(f, "CMP/PL  {}", rn),
            InstructionKind::CmpPz { rn } => write!(f, "CMP/PZ  {}", rn),
            InstructionKind::CmpStr { rm, rn } => write!(f, "CMP/STR {},{}", rm, rn),

            // === Branches ===
            InstructionKind::Bt { disp } => {
                write!(f, "BT      {}", (*disp as i32) * 2)
            }
            InstructionKind::Bf { disp } => {
                write!(f, "BF      {}", (*disp as i32) * 2)
            }
            InstructionKind::Bts { disp } => {
                write!(f, "BT/S    {}", (*disp as i32) * 2)
            }
            InstructionKind::Bfs { disp } => {
                write!(f, "BF/S    {}", (*disp as i32) * 2)
            }
            InstructionKind::Bra { disp } => {
                write!(f, "BRA     {}", (*disp as i32) * 2)
            }
            InstructionKind::Bsr { disp } => {
                write!(f, "BSR     {}", (*disp as i32) * 2)
            }
            InstructionKind::Jmp { rm } => write!(f, "JMP     @{}", rm),
            InstructionKind::Jsr { rm } => write!(f, "JSR     @{}", rm),
            InstructionKind::Rts => write!(f, "RTS"),
            InstructionKind::Rte => write!(f, "RTE"),

            // === System register transfer ===
            InstructionKind::LdcGbr { rm } => write!(f, "LDC     {},GBR", rm),
            InstructionKind::LdcVbr { rm } => write!(f, "LDC     {},VBR", rm),
            InstructionKind::LdcSr { rm } => write!(f, "LDC     {},SR", rm),
            InstructionKind::LdcLGbr { rm } => write!(f, "LDC.L   @{}+,GBR", rm),
            InstructionKind::LdcLVbr { rm } => write!(f, "LDC.L   @{}+,VBR", rm),
            InstructionKind::LdcLSr { rm } => write!(f, "LDC.L   @{}+,SR", rm),
            InstructionKind::StcGbr { rn } => write!(f, "STC     GBR,{}", rn),
            InstructionKind::StcVbr { rn } => write!(f, "STC     VBR,{}", rn),
            InstructionKind::StcSr { rn } => write!(f, "STC     SR,{}", rn),
            InstructionKind::StcLGbr { rn } => write!(f, "STC.L   GBR,@-{}", rn),
            InstructionKind::StcLVbr { rn } => write!(f, "STC.L   VBR,@-{}", rn),
            InstructionKind::StcLSr { rn } => write!(f, "STC.L   SR,@-{}", rn),
            InstructionKind::LdsMach { rm } => write!(f, "LDS     {},MACH", rm),
            InstructionKind::LdsMacl { rm } => write!(f, "LDS     {},MACL", rm),
            InstructionKind::LdsPr { rm } => write!(f, "LDS     {},PR", rm),
            InstructionKind::LdsLMach { rm } => write!(f, "LDS.L   @{}+,MACH", rm),
            InstructionKind::LdsLMacl { rm } => write!(f, "LDS.L   @{}+,MACL", rm),
            InstructionKind::LdsLPr { rm } => write!(f, "LDS.L   @{}+,PR", rm),
            InstructionKind::StsMach { rn } => write!(f, "STS     MACH,{}", rn),
            InstructionKind::StsMacl { rn } => write!(f, "STS     MACL,{}", rn),
            InstructionKind::StsPr { rn } => write!(f, "STS     PR,{}", rn),
            InstructionKind::StsLMach { rn } => write!(f, "STS.L   MACH,@-{}", rn),
            InstructionKind::StsLMacl { rn } => write!(f, "STS.L   MACL,@-{}", rn),
            InstructionKind::StsLPr { rn } => write!(f, "STS.L   PR,@-{}", rn),

            // === Special ===
            InstructionKind::Nop => write!(f, "NOP"),
            InstructionKind::Sleep => write!(f, "SLEEP"),
            InstructionKind::SetT => write!(f, "SETT"),
            InstructionKind::ClrT => write!(f, "CLRT"),
            InstructionKind::ClrMac => write!(f, "CLRMAC"),
            InstructionKind::Trapa { imm } => write!(f, "TRAPA   #{}", imm),
            InstructionKind::Bsrf { rm } => write!(f, "BSRF    {}", rm),
            InstructionKind::Braf { rm } => write!(f, "BRAF    {}", rm),
            InstructionKind::Tas { rn } => write!(f, "TAS.B   @{}", rn),

            // === Unknown ===
            InstructionKind::Unknown { opcode } => write!(f, ".word   0x{:04X}", opcode),
        }
    }
}

#[cfg(test)]
#[path = "display_tests.rs"]
mod tests;
