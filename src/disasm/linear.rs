use super::address::{AddressSpace, VAddr};
use crate::sh2::{self, Instruction};

/// A single disassembled line.
#[derive(Debug, Clone)]
pub struct DisasmLine {
    /// Address of this instruction.
    pub addr: VAddr,
    /// Raw 16-bit opcode.
    pub opcode: u16,
    /// Decoded instruction.
    pub instruction: Instruction,
    /// PC-relative branch target address, if applicable.
    pub branch_target: Option<VAddr>,
    /// Literal pool reference address, if applicable.
    pub literal_pool_addr: Option<VAddr>,
    /// 32-bit value read from the literal pool, if applicable.
    pub literal_pool_value: Option<u32>,
}

/// Linearly disassemble an address range, decoding every 2-byte word as an instruction.
pub fn disassemble_linear(space: &AddressSpace, start: VAddr, end: VAddr) -> Vec<DisasmLine> {
    let mut lines = Vec::new();
    let mut pc = start;

    while pc < end {
        let Some(opcode) = space.read_u16_be(pc) else {
            break;
        };
        let inst = sh2::decode(opcode);

        let branch_target = inst.branch_target(pc);
        let literal_pool_addr = inst.literal_pool_addr(pc);
        let literal_pool_value = literal_pool_addr.and_then(|a| space.read_u32_be(a));

        lines.push(DisasmLine {
            addr: pc,
            opcode,
            instruction: inst,
            branch_target,
            literal_pool_addr,
            literal_pool_value,
        });

        pc += 2;
    }

    lines
}

impl std::fmt::Display for DisasmLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:08X}: {:04X}  {}",
            self.addr, self.opcode, self.instruction
        )?;

        if let Some(target) = self.branch_target {
            write!(f, "  ; -> 0x{target:08X}")?;
        }

        if let Some(value) = self.literal_pool_value {
            write!(f, "  ; =0x{value:08X}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "linear_tests.rs"]
mod tests;
