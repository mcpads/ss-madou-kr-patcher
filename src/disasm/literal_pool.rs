use super::address::VAddr;
use super::analysis::{AddrType, AnalysisDb};

/// Interpretation of a literal pool value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralPoolInterpretation {
    /// Code address (function pointer, jump table entry).
    CodePointer(VAddr),
    /// Data address (string pointer, table pointer).
    DataPointer(VAddr),
    /// Hardware register address.
    HardwareRegister(VAddr, &'static str),
    /// Immediate constant (bitmask, magic number, etc.).
    Constant(u32),
}

pub struct LiteralPoolAnalyzer;

impl LiteralPoolAnalyzer {
    /// Interpret a literal pool value using context from the analysis DB.
    pub fn interpret(value: u32, db: &AnalysisDb) -> LiteralPoolInterpretation {
        // 1. Check hardware register addresses first
        if let Some(name) = Self::hardware_register_name(value) {
            return LiteralPoolInterpretation::HardwareRegister(value, name);
        }
        // 2. Work RAM High: code pointer if already classified as code
        if value >= 0x0600_0000 && value < 0x0610_0000 && value % 2 == 0 {
            if matches!(
                db.addr_types.get(&value),
                Some(AddrType::Code) | Some(AddrType::FunctionEntry)
            ) {
                return LiteralPoolInterpretation::CodePointer(value);
            }
            return LiteralPoolInterpretation::DataPointer(value);
        }
        // 3. Work RAM Low
        if value >= 0x0020_0000 && value < 0x0030_0000 {
            return LiteralPoolInterpretation::DataPointer(value);
        }
        // 4. Everything else is a constant
        LiteralPoolInterpretation::Constant(value)
    }

    /// Saturn hardware register name lookup.
    fn hardware_register_name(addr: u32) -> Option<&'static str> {
        // This covers the most commonly used Saturn I/O registers.
        // Ranges: SMPC 0x0010_xxxx, VDP1 0x05D0_xxxx, VDP2 0x05E0/05F8_xxxx,
        //         SCU 0x05FE_xxxx, SCSP 0x05A0_xxxx
        match addr {
            // SMPC
            0x0010_0001 => Some("SMPC_IREG0"),
            0x0010_0003 => Some("SMPC_IREG1"),
            0x0010_0005 => Some("SMPC_IREG2"),
            0x0010_0007 => Some("SMPC_IREG3"),
            0x0010_0009 => Some("SMPC_IREG4"),
            0x0010_000B => Some("SMPC_IREG5"),
            0x0010_000D => Some("SMPC_IREG6"),
            0x0010_001F => Some("SMPC_COMREG"),
            0x0010_0021 => Some("SMPC_OREG0"),
            0x0010_0061 => Some("SMPC_SR"),
            0x0010_0063 => Some("SMPC_SF"),
            0x0010_0075 => Some("SMPC_PDR1"),
            0x0010_0077 => Some("SMPC_PDR2"),
            0x0010_0079 => Some("SMPC_DDR1"),
            0x0010_007B => Some("SMPC_DDR2"),
            // VDP1
            0x05D0_0000 => Some("VDP1_TVMR"),
            0x05D0_0002 => Some("VDP1_FBCR"),
            0x05D0_0004 => Some("VDP1_PTMR"),
            0x05D0_0006 => Some("VDP1_EWDR"),
            0x05D0_0008 => Some("VDP1_EWLR"),
            0x05D0_000A => Some("VDP1_EWRR"),
            0x05D0_000C => Some("VDP1_ENDR"),
            0x05D0_0010 => Some("VDP1_EDSR"),
            0x05D0_0012 => Some("VDP1_LOPR"),
            0x05D0_0014 => Some("VDP1_COPR"),
            0x05D0_0016 => Some("VDP1_MODR"),
            // VDP2
            0x05F8_0000 => Some("VDP2_TVMD"),
            0x05F8_0002 => Some("VDP2_EXTEN"),
            0x05F8_0004 => Some("VDP2_TVSTAT"),
            0x05F8_0006 => Some("VDP2_VRSIZE"),
            0x05F8_000E => Some("VDP2_BGON"),
            0x05F8_0020 => Some("VDP2_CHCTLA"),
            0x05F8_0022 => Some("VDP2_CHCTLB"),
            0x05F8_00E0 => Some("VDP2_SPCTL"),
            0x05F8_00F8 => Some("VDP2_CLOFEN"),
            // SCU
            0x05FE_0000 => Some("SCU_D0R"),
            0x05FE_0008 => Some("SCU_D0C"),
            0x05FE_0010 => Some("SCU_D0MD"),
            0x05FE_0080 => Some("SCU_DSTA"),
            0x05FE_00A0 => Some("SCU_IMS"),
            0x05FE_00A4 => Some("SCU_IST"),
            // VDP1 VRAM
            0x05C0_0000 => Some("VDP1_VRAM"),
            // VDP2 VRAM
            0x05E0_0000 => Some("VDP2_VRAM"),
            // VDP2 CRAM
            0x05F0_0000 => Some("VDP2_CRAM"),
            _ => None,
        }
    }

    /// Classify all literal pool values in the database and return statistics.
    pub fn classify_all(db: &AnalysisDb) -> LiteralPoolStats {
        let mut stats = LiteralPoolStats::default();
        for &value in db.literal_pool_values.values() {
            match Self::interpret(value, db) {
                LiteralPoolInterpretation::CodePointer(_) => stats.code_pointers += 1,
                LiteralPoolInterpretation::DataPointer(_) => stats.data_pointers += 1,
                LiteralPoolInterpretation::HardwareRegister(_, _) => stats.hardware_regs += 1,
                LiteralPoolInterpretation::Constant(_) => stats.constants += 1,
            }
        }
        stats.total = db.literal_pool_values.len();
        stats
    }
}

/// Statistics about literal pool value classification.
#[derive(Debug, Default)]
pub struct LiteralPoolStats {
    pub total: usize,
    pub code_pointers: usize,
    pub data_pointers: usize,
    pub hardware_regs: usize,
    pub constants: usize,
}

impl std::fmt::Display for LiteralPoolStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Literal pool: {} total ({} code ptrs, {} data ptrs, {} hw regs, {} constants)",
            self.total, self.code_pointers, self.data_pointers, self.hardware_regs, self.constants
        )
    }
}

#[cfg(test)]
#[path = "literal_pool_tests.rs"]
mod tests;
