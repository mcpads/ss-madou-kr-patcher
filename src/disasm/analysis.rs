use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::address::VAddr;
use super::linear::DisasmLine;

/// Classification of an address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrType {
    /// Executable code.
    Code,
    /// Literal pool (32-bit constant loaded via PC-relative MOV.L).
    LiteralPool,
    /// Known data (not code).
    Data,
    /// Function entry point.
    FunctionEntry,
}

/// Cross-reference kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XRefKind {
    /// Unconditional branch (BRA, JMP).
    Branch,
    /// Conditional branch (BT, BF, BT/S, BF/S).
    ConditionalBranch,
    /// Subroutine call (BSR, JSR).
    Call,
    /// Literal pool reference (MOV.L @(disp,PC), Rn).
    LiteralPoolRef,
    /// Indirect reference: literal pool value points to this address.
    IndirectRef,
}

/// A single cross-reference entry.
#[derive(Debug, Clone)]
pub struct XRef {
    /// Source address (the instruction making the reference).
    pub from: VAddr,
    /// Target address (the address being referenced).
    pub to: VAddr,
    /// What kind of reference.
    pub kind: XRefKind,
}

/// Database storing all disassembly analysis results.
pub struct AnalysisDb {
    /// Address → type classification.
    pub addr_types: BTreeMap<VAddr, AddrType>,
    /// Address → disassembled instruction.
    pub instructions: BTreeMap<VAddr, DisasmLine>,
    /// Target address → incoming cross-references.
    pub xrefs_to: HashMap<VAddr, Vec<XRef>>,
    /// Source address → outgoing cross-references.
    pub xrefs_from: HashMap<VAddr, Vec<XRef>>,
    /// Identified function entry points.
    pub functions: BTreeSet<VAddr>,
    /// Literal pool address → 32-bit value.
    pub literal_pool_values: BTreeMap<VAddr, u32>,
    /// Address → label name.
    pub labels: BTreeMap<VAddr, String>,
}

impl AnalysisDb {
    pub fn new() -> Self {
        Self {
            addr_types: BTreeMap::new(),
            instructions: BTreeMap::new(),
            xrefs_to: HashMap::new(),
            xrefs_from: HashMap::new(),
            functions: BTreeSet::new(),
            literal_pool_values: BTreeMap::new(),
            labels: BTreeMap::new(),
        }
    }

    /// Mark an address as code. Does not override FunctionEntry.
    pub fn mark_code(&mut self, addr: VAddr) {
        self.addr_types
            .entry(addr)
            .or_insert(AddrType::Code);
    }

    /// Mark an address as a literal pool entry with its value.
    pub fn mark_literal_pool(&mut self, addr: VAddr, value: u32) {
        self.addr_types.insert(addr, AddrType::LiteralPool);
        self.literal_pool_values.insert(addr, value);
    }

    /// Mark an address as a function entry point.
    pub fn mark_function(&mut self, addr: VAddr, name: Option<String>) {
        self.addr_types.insert(addr, AddrType::FunctionEntry);
        self.functions.insert(addr);
        if let Some(n) = name {
            self.labels.insert(addr, n);
        } else if !self.labels.contains_key(&addr) {
            self.labels.insert(addr, format!("sub_{addr:08X}"));
        }
    }

    pub fn add_xref(&mut self, xref: XRef) {
        self.xrefs_to
            .entry(xref.to)
            .or_default()
            .push(xref.clone());
        self.xrefs_from
            .entry(xref.from)
            .or_default()
            .push(xref);
    }

    pub fn xrefs_to(&self, addr: VAddr) -> &[XRef] {
        self.xrefs_to.get(&addr).map_or(&[], |v| v.as_slice())
    }

    pub fn xrefs_from(&self, addr: VAddr) -> &[XRef] {
        self.xrefs_from.get(&addr).map_or(&[], |v| v.as_slice())
    }

    /// Number of addresses classified as code or function entries.
    pub fn code_count(&self) -> usize {
        self.addr_types
            .values()
            .filter(|t| matches!(t, AddrType::Code | AddrType::FunctionEntry))
            .count()
    }

    /// Find all literal pool entries that contain a specific value.
    /// Returns (literal_pool_addr, instructions_that_reference_it).
    pub fn find_literal_pool_by_value(&self, target: u32) -> Vec<(VAddr, Vec<&XRef>)> {
        let mut results = Vec::new();
        for (&pool_addr, &value) in &self.literal_pool_values {
            if value == target {
                let refs = self.xrefs_to(pool_addr).to_vec();
                let ref_borrows: Vec<&XRef> = self
                    .xrefs_to
                    .get(&pool_addr)
                    .map_or(Vec::new(), |v| v.iter().collect());
                let _ = refs; // used only for the pattern
                results.push((pool_addr, ref_borrows));
            }
        }
        results.sort_by_key(|(addr, _)| *addr);
        results
    }

    /// Find all callers of a function (JSR/BSR xrefs).
    pub fn find_callers(&self, func_addr: VAddr) -> Vec<VAddr> {
        self.xrefs_to(func_addr)
            .iter()
            .filter(|x| matches!(x.kind, XRefKind::Call))
            .map(|x| x.from)
            .collect()
    }

    /// Find the function that contains a given address.
    /// Returns the highest function entry point that is <= addr.
    pub fn containing_function(&self, addr: VAddr) -> Option<VAddr> {
        self.functions.range(..=addr).next_back().copied()
    }

    /// Get all instructions belonging to a function (from entry to next function or gap).
    pub fn function_instructions(&self, func_addr: VAddr) -> Vec<(&VAddr, &DisasmLine)> {
        let next_func = self.functions.range((
            std::ops::Bound::Excluded(func_addr),
            std::ops::Bound::Unbounded,
        )).next().copied();

        self.instructions
            .range(func_addr..)
            .take_while(|&(&addr, _)| {
                addr == func_addr || next_func.map_or(true, |nf| addr < nf)
            })
            .collect()
    }
}

impl Default for AnalysisDb {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod tests;
