use std::io::Write;

use crate::disasm::analysis::AnalysisDb;
use crate::disasm::linear::DisasmLine;

/// Writes a formatted assembly listing to any `Write` destination.
pub struct ListingWriter<'a> {
    db: &'a AnalysisDb,
}

impl<'a> ListingWriter<'a> {
    pub fn new(db: &'a AnalysisDb) -> Self {
        Self { db }
    }

    /// Write the full listing for all instructions in address order.
    pub fn write_listing<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        writeln!(w, "; === Madou Monogatari (T-6607G) SH-2 Disassembly ===")?;
        writeln!(
            w,
            "; Functions: {} | Code addresses: {}",
            self.db.functions.len(),
            self.db.code_count()
        )?;
        writeln!(w)?;

        let mut in_literal_pool = false;

        for (&addr, line) in &self.db.instructions {
            // Check if this address is a function entry - print header
            if self.db.functions.contains(&addr) {
                if in_literal_pool {
                    writeln!(w)?;
                    in_literal_pool = false;
                }
                writeln!(w)?;
                let label = self.db.labels.get(&addr).map_or_else(
                    || format!("sub_{addr:08X}"),
                    |n| n.clone(),
                );
                writeln!(w, "; -------- {label} --------")?;

                // Print incoming xrefs
                let xrefs = self.db.xrefs_to(addr);
                if !xrefs.is_empty() {
                    let refs: Vec<String> = xrefs
                        .iter()
                        .map(|x| format!("{:08X}", x.from))
                        .collect();
                    writeln!(w, "; xrefs: {}", refs.join(", "))?;
                }
            } else if let Some(label) = self.db.labels.get(&addr) {
                // Non-function label
                writeln!(w, "{label}:")?;
            }

            // Print the instruction line
            self.write_instruction(w, line)?;
        }

        // Print literal pool entries that aren't also instructions
        let pool_only: Vec<_> = self
            .db
            .literal_pool_values
            .iter()
            .filter(|(addr, _)| !self.db.instructions.contains_key(addr))
            .collect();

        if !pool_only.is_empty() {
            writeln!(w)?;
            writeln!(w, "; -------- literal pool data --------")?;
            for (addr, value) in &pool_only {
                write!(w, "{addr:08X}: {value:08X}  .long   0x{value:08X}")?;
                // Annotate if value points to a known function
                if let Some(label) = self.db.labels.get(*value) {
                    write!(w, "       ; -> {label}")?;
                }
                writeln!(w)?;
            }
        }

        Ok(())
    }

    fn write_instruction<W: Write>(&self, w: &mut W, line: &DisasmLine) -> std::io::Result<()> {
        write!(
            w,
            "{:08X}: {:04X}  {}",
            line.addr, line.opcode, line.instruction
        )?;

        if let Some(target) = line.branch_target {
            write!(w, "  ; -> 0x{target:08X}")?;
            if let Some(label) = self.db.labels.get(&target) {
                write!(w, " ({label})")?;
            }
        }

        if let Some(value) = line.literal_pool_value {
            write!(w, "  ; =0x{value:08X}")?;
            if let Some(label) = self.db.labels.get(&value) {
                write!(w, " -> {label}")?;
            }
        }

        writeln!(w)?;
        Ok(())
    }

    /// Write a simple linear listing (no analysis, just instructions).
    pub fn write_linear_listing<W: Write>(
        w: &mut W,
        lines: &[DisasmLine],
    ) -> std::io::Result<()> {
        for line in lines {
            writeln!(w, "{line}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
