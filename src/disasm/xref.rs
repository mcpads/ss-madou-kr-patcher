use super::address::{AddressSpace, VAddr};
use super::analysis::{AnalysisDb, XRef, XRefKind};
use super::literal_pool::{LiteralPoolAnalyzer, LiteralPoolInterpretation};
use crate::text::sjis::{is_sjis_lead_byte, is_sjis_trail_byte};

/// A candidate text pointer found via literal pool analysis.
#[derive(Debug)]
pub struct TextPointerCandidate {
    /// Address of the literal pool entry containing the pointer.
    pub pointer_addr: VAddr,
    /// Address the pointer points to (potential text data).
    pub text_addr: VAddr,
    /// Instructions that reference this literal pool entry.
    pub referencing_instructions: Vec<XRef>,
}

/// Analyzer for finding potential text data references.
pub struct TextRefAnalyzer;

impl TextRefAnalyzer {
    /// Find literal pool values that point to potential text data.
    pub fn find_potential_text_pointers(
        db: &AnalysisDb,
        space: &AddressSpace,
    ) -> Vec<TextPointerCandidate> {
        let mut candidates = Vec::new();

        for (&pool_addr, &value) in &db.literal_pool_values {
            let interp = LiteralPoolAnalyzer::interpret(value, db);
            if let LiteralPoolInterpretation::DataPointer(data_addr) = interp {
                if Self::looks_like_text(space, data_addr) {
                    candidates.push(TextPointerCandidate {
                        pointer_addr: pool_addr,
                        text_addr: data_addr,
                        referencing_instructions: db.xrefs_to(pool_addr).to_vec(),
                    });
                }
            }
        }

        candidates.sort_by_key(|c| c.text_addr);
        candidates
    }

    /// Heuristic: does the data at `addr` look like Shift-JIS text?
    fn looks_like_text(space: &AddressSpace, addr: VAddr) -> bool {
        let mut offset = addr;
        let mut char_count = 0;

        for _ in 0..256 {
            let Some(b) = space.read_u8(offset) else {
                break;
            };
            if b == 0x00 {
                break; // null terminator
            }

            if is_sjis_lead_byte(b) {
                let Some(b2) = space.read_u8(offset + 1) else {
                    break;
                };
                if is_sjis_trail_byte(b2) {
                    char_count += 1;
                    offset += 2;
                    continue;
                }
                break; // invalid trail byte
            } else if (0x20..=0x7E).contains(&b) || (0xA1..=0xDF).contains(&b) {
                // ASCII printable or half-width katakana
                char_count += 1;
                offset += 1;
                continue;
            }
            break; // invalid byte
        }

        char_count >= 3
    }
}

/// Cross-reference report generation.
pub struct XRefReport;

impl XRefReport {
    /// Generate a summary of cross-references grouped by kind.
    pub fn summary(db: &AnalysisDb) -> XRefSummary {
        let mut summary = XRefSummary::default();
        for refs in db.xrefs_to.values() {
            for xref in refs {
                match xref.kind {
                    XRefKind::Branch => summary.branches += 1,
                    XRefKind::ConditionalBranch => summary.conditional_branches += 1,
                    XRefKind::Call => summary.calls += 1,
                    XRefKind::LiteralPoolRef => summary.literal_pool_refs += 1,
                    XRefKind::IndirectRef => summary.indirect_refs += 1,
                }
                summary.total += 1;
            }
        }
        summary
    }

    /// Format cross-references for a specific address.
    pub fn format_xrefs_to(db: &AnalysisDb, addr: VAddr) -> String {
        let xrefs = db.xrefs_to(addr);
        if xrefs.is_empty() {
            return String::new();
        }

        let mut parts: Vec<String> = xrefs
            .iter()
            .map(|x| {
                let kind_str = match x.kind {
                    XRefKind::Branch => "branch",
                    XRefKind::ConditionalBranch => "cond",
                    XRefKind::Call => "call",
                    XRefKind::LiteralPoolRef => "pool",
                    XRefKind::IndirectRef => "indirect",
                };
                format!("{kind_str}@{:08X}", x.from)
            })
            .collect();
        parts.sort();
        format!("; xrefs: {}", parts.join(", "))
    }
}

/// Summary of cross-reference counts.
#[derive(Debug, Default)]
pub struct XRefSummary {
    pub total: usize,
    pub branches: usize,
    pub conditional_branches: usize,
    pub calls: usize,
    pub literal_pool_refs: usize,
    pub indirect_refs: usize,
}

impl std::fmt::Display for XRefSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Xrefs: {} total ({} branches, {} cond, {} calls, {} pool refs, {} indirect)",
            self.total,
            self.branches,
            self.conditional_branches,
            self.calls,
            self.literal_pool_refs,
            self.indirect_refs
        )
    }
}

#[cfg(test)]
#[path = "xref_tests.rs"]
mod tests;
