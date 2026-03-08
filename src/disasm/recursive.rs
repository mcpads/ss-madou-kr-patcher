use std::collections::{HashSet, VecDeque};

use super::address::{AddressSpace, VAddr};
use super::analysis::{AnalysisDb, XRef, XRefKind};
use super::linear::DisasmLine;
use crate::sh2::{self, FlowKind, InstructionKind, Reg};

/// Recursive (control-flow-following) disassembler.
///
/// Starting from entry points, follows all reachable code paths, correctly
/// handling delay slots, literal pool references, and indirect calls.
pub struct RecursiveDisassembler<'a> {
    space: &'a AddressSpace,
    db: AnalysisDb,
    queue: VecDeque<VAddr>,
    visited: HashSet<VAddr>,
}

impl<'a> RecursiveDisassembler<'a> {
    pub fn new(space: &'a AddressSpace) -> Self {
        Self {
            space,
            db: AnalysisDb::new(),
            queue: VecDeque::new(),
            visited: HashSet::new(),
        }
    }

    /// Add an entry point to analyze (e.g. 1st Read Address, interrupt vectors).
    pub fn add_entry_point(&mut self, addr: VAddr, name: Option<String>) {
        self.db.mark_function(addr, name);
        self.queue.push_back(addr);
    }

    /// Run the recursive disassembly until the queue is exhausted.
    pub fn run(mut self) -> AnalysisDb {
        while let Some(addr) = self.queue.pop_front() {
            if self.visited.contains(&addr) {
                continue;
            }
            self.analyze_block(addr);
        }
        self.db
    }

    /// Run with aggressive entry point discovery: after initial pass,
    /// scan literal pool values for code-like addresses and re-run.
    pub fn run_deep(mut self) -> AnalysisDb {
        // First pass: normal recursive disassembly
        while let Some(addr) = self.queue.pop_front() {
            if self.visited.contains(&addr) {
                continue;
            }
            self.analyze_block(addr);
        }

        // Second pass: discover new entry points from literal pool values
        // that look like code addresses but weren't reached.
        loop {
            let new_entries: Vec<VAddr> = self
                .db
                .literal_pool_values
                .values()
                .filter(|&&v| is_plausible_code_addr(v) && !self.visited.contains(&v))
                .copied()
                .collect();

            if new_entries.is_empty() {
                break;
            }

            for addr in new_entries {
                if !self.visited.contains(&addr) {
                    self.db.mark_function(addr, None);
                    self.queue.push_back(addr);
                }
            }

            while let Some(addr) = self.queue.pop_front() {
                if self.visited.contains(&addr) {
                    continue;
                }
                self.analyze_block(addr);
            }
        }

        self.db
    }

    /// Analyze a basic block starting at `start`, following fall-through and
    /// queuing branch targets.
    fn analyze_block(&mut self, start: VAddr) {
        let mut pc = start;

        loop {
            if self.visited.contains(&pc) {
                break;
            }
            self.visited.insert(pc);

            let Some(opcode) = self.space.read_u16_be(pc) else {
                break;
            };
            let inst = sh2::decode(opcode);

            self.db.mark_code(pc);

            let branch_target = inst.branch_target(pc);
            let literal_pool_addr = inst.literal_pool_addr(pc);
            let literal_pool_value = literal_pool_addr.and_then(|a| self.space.read_u32_be(a));

            // Record literal pool reference.
            if let Some(pool_addr) = literal_pool_addr {
                if let Some(value) = literal_pool_value {
                    self.db.mark_literal_pool(pool_addr, value);
                    self.db.add_xref(XRef {
                        from: pc,
                        to: pool_addr,
                        kind: XRefKind::LiteralPoolRef,
                    });

                    // If the literal pool value looks like a code address, record indirect ref.
                    if is_plausible_code_addr(value) {
                        self.db.add_xref(XRef {
                            from: pool_addr,
                            to: value,
                            kind: XRefKind::IndirectRef,
                        });
                    }
                }
            }

            let line = DisasmLine {
                addr: pc,
                opcode,
                instruction: inst.clone(),
                branch_target,
                literal_pool_addr,
                literal_pool_value,
            };
            self.db.instructions.insert(pc, line);

            match inst.flow {
                FlowKind::Normal => {
                    pc += 2;
                    continue;
                }

                FlowKind::ConditionalBranch => {
                    // BT, BF: no delay slot.
                    // Queue the branch target and continue with fall-through.
                    if let Some(target) = branch_target {
                        self.db.add_xref(XRef {
                            from: pc,
                            to: target,
                            kind: XRefKind::ConditionalBranch,
                        });
                        self.enqueue(target);
                    }
                    pc += 2;
                    continue;
                }

                FlowKind::ConditionalBranchDelayed => {
                    // BT/S, BF/S: has delay slot.
                    self.decode_delay_slot(pc);

                    if let Some(target) = branch_target {
                        self.db.add_xref(XRef {
                            from: pc,
                            to: target,
                            kind: XRefKind::ConditionalBranch,
                        });
                        self.enqueue(target);
                    }
                    pc += 4; // skip past delay slot for fall-through
                    continue;
                }

                FlowKind::UnconditionalBranch => {
                    // BRA, JMP: has delay slot, no fall-through.
                    self.decode_delay_slot(pc);

                    if let Some(target) = branch_target {
                        self.db.add_xref(XRef {
                            from: pc,
                            to: target,
                            kind: XRefKind::Branch,
                        });
                        self.enqueue(target);
                    }

                    // For JMP @Rm, try to resolve via literal pool backtracking.
                    if let InstructionKind::Jmp { rm } = inst.kind {
                        if let Some(target) = self.resolve_register_target(pc, rm) {
                            self.db.add_xref(XRef {
                                from: pc,
                                to: target,
                                kind: XRefKind::Branch,
                            });
                            self.enqueue(target);
                        }
                    }

                    break; // no fall-through
                }

                FlowKind::Call => {
                    // BSR, JSR: has delay slot, fall-through after delay slot.
                    self.decode_delay_slot(pc);

                    if let Some(target) = branch_target {
                        self.db.mark_function(target, None);
                        self.db.add_xref(XRef {
                            from: pc,
                            to: target,
                            kind: XRefKind::Call,
                        });
                        self.enqueue(target);
                    }

                    // For JSR @Rm, try to resolve via literal pool backtracking.
                    if let InstructionKind::Jsr { rm } = inst.kind {
                        if let Some(target) = self.resolve_register_target(pc, rm) {
                            self.db.mark_function(target, None);
                            self.db.add_xref(XRef {
                                from: pc,
                                to: target,
                                kind: XRefKind::Call,
                            });
                            self.enqueue(target);
                        }
                    }

                    pc += 4; // skip past delay slot for fall-through
                    continue;
                }

                FlowKind::Return | FlowKind::ExceptionReturn => {
                    // RTS, RTE: has delay slot, no fall-through.
                    self.decode_delay_slot(pc);
                    break;
                }
            }
        }
    }

    /// Decode and record the delay slot instruction at `branch_pc + 2`.
    fn decode_delay_slot(&mut self, branch_pc: VAddr) {
        let delay_pc = branch_pc + 2;
        if self.visited.contains(&delay_pc) {
            return;
        }
        self.visited.insert(delay_pc);

        if let Some(opcode) = self.space.read_u16_be(delay_pc) {
            let inst = sh2::decode(opcode);
            self.db.mark_code(delay_pc);

            let literal_pool_addr = inst.literal_pool_addr(delay_pc);
            let literal_pool_value = literal_pool_addr.and_then(|a| self.space.read_u32_be(a));

            if let Some(pool_addr) = literal_pool_addr {
                if let Some(value) = literal_pool_value {
                    self.db.mark_literal_pool(pool_addr, value);
                    self.db.add_xref(XRef {
                        from: delay_pc,
                        to: pool_addr,
                        kind: XRefKind::LiteralPoolRef,
                    });
                }
            }

            let line = DisasmLine {
                addr: delay_pc,
                opcode,
                instruction: inst,
                branch_target: None,
                literal_pool_addr,
                literal_pool_value,
            };
            self.db.instructions.insert(delay_pc, line);
        }
    }

    /// Try to resolve `JSR @Rm` / `JMP @Rm` by backtracking to find a
    /// `MOV.L @(disp,PC), Rm` that loaded the register.
    fn resolve_register_target(&self, jsr_pc: VAddr, target_reg: Reg) -> Option<VAddr> {
        // Search backwards up to 20 instructions (40 bytes).
        for offset in (2..=40).step_by(2) {
            let check_addr = jsr_pc.checked_sub(offset)?;

            if let Some(line) = self.db.instructions.get(&check_addr) {
                match &line.instruction.kind {
                    InstructionKind::MovLPcRel { rn, .. } if *rn == target_reg => {
                        return line.literal_pool_value;
                    }
                    _ => {
                        if instruction_writes_reg(&line.instruction.kind, target_reg) {
                            return None; // register overwritten by something else
                        }
                    }
                }
            }
        }
        None
    }

    fn enqueue(&mut self, addr: VAddr) {
        if !self.visited.contains(&addr) {
            self.queue.push_back(addr);
        }
    }
}

/// Check if a value looks like a plausible code address in Work RAM High.
fn is_plausible_code_addr(addr: VAddr) -> bool {
    addr >= 0x0600_0000 && addr < 0x0610_0000 && addr % 2 == 0
}

/// Check if an instruction writes to the given register.
fn instruction_writes_reg(kind: &InstructionKind, reg: Reg) -> bool {
    match kind {
        // MOV / ALU / extension / swap variants that write to rn
        InstructionKind::MovImm { rn, .. }
        | InstructionKind::MovWPcRel { rn, .. }
        | InstructionKind::MovLPcRel { rn, .. }
        | InstructionKind::MovReg { rn, .. }
        | InstructionKind::MovBLoad { rn, .. }
        | InstructionKind::MovWLoad { rn, .. }
        | InstructionKind::MovLLoad { rn, .. }
        | InstructionKind::MovBLoadPostInc { rn, .. }
        | InstructionKind::MovWLoadPostInc { rn, .. }
        | InstructionKind::MovLLoadPostInc { rn, .. }
        | InstructionKind::MovBR0LoadIndexed { rn, .. }
        | InstructionKind::MovWR0LoadIndexed { rn, .. }
        | InstructionKind::MovLR0LoadIndexed { rn, .. }
        | InstructionKind::MovLDispLoad { rn, .. }
        | InstructionKind::Add { rn, .. }
        | InstructionKind::AddImm { rn, .. }
        | InstructionKind::Addc { rn, .. }
        | InstructionKind::Addv { rn, .. }
        | InstructionKind::Sub { rn, .. }
        | InstructionKind::Subc { rn, .. }
        | InstructionKind::Subv { rn, .. }
        | InstructionKind::Neg { rn, .. }
        | InstructionKind::Negc { rn, .. }
        | InstructionKind::Not { rn, .. }
        | InstructionKind::And { rn, .. }
        | InstructionKind::Or { rn, .. }
        | InstructionKind::Xor { rn, .. }
        | InstructionKind::Xtrct { rn, .. }
        | InstructionKind::Div1 { rn, .. }
        | InstructionKind::ExtsB { rn, .. }
        | InstructionKind::ExtsW { rn, .. }
        | InstructionKind::ExtuB { rn, .. }
        | InstructionKind::ExtuW { rn, .. }
        | InstructionKind::SwapB { rn, .. }
        | InstructionKind::SwapW { rn, .. }
        | InstructionKind::Dt { rn }
        | InstructionKind::MovT { rn }
        // Shift / rotate — all write to rn
        | InstructionKind::Shll { rn }
        | InstructionKind::Shlr { rn }
        | InstructionKind::Shal { rn }
        | InstructionKind::Shar { rn }
        | InstructionKind::Shll2 { rn }
        | InstructionKind::Shlr2 { rn }
        | InstructionKind::Shll8 { rn }
        | InstructionKind::Shlr8 { rn }
        | InstructionKind::Shll16 { rn }
        | InstructionKind::Shlr16 { rn }
        | InstructionKind::Rotl { rn }
        | InstructionKind::Rotr { rn }
        | InstructionKind::Rotcl { rn }
        | InstructionKind::Rotcr { rn }
        // STC / STS — store system register to rn
        | InstructionKind::StcGbr { rn }
        | InstructionKind::StcVbr { rn }
        | InstructionKind::StcSr { rn }
        | InstructionKind::StsMach { rn }
        | InstructionKind::StsMacl { rn }
        | InstructionKind::StsPr { rn } => *rn == reg,

        // Instructions that write to R0
        InstructionKind::MovBDispLoad { .. }
        | InstructionKind::MovWDispLoad { .. }
        | InstructionKind::Mova { .. }
        | InstructionKind::MovBGbr { store: false, .. }
        | InstructionKind::MovWGbr { store: false, .. }
        | InstructionKind::MovLGbr { store: false, .. } => reg == Reg::R0,

        _ => false,
    }
}

#[cfg(test)]
#[path = "recursive_tests.rs"]
mod tests;
