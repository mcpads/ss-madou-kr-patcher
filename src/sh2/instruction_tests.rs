use super::*;

#[test]
fn test_reg_constants() {
    assert_eq!(Reg::R0.0, 0);
    assert_eq!(Reg::SP.0, 15);
}

#[test]
fn test_reg_display() {
    assert_eq!(format!("{}", Reg(0)), "R0");
    assert_eq!(format!("{}", Reg(15)), "R15");
    assert_eq!(format!("{}", Reg(8)), "R8");
}

#[test]
fn test_has_delay_slot() {
    // UnconditionalBranch has delay slot
    let inst = Instruction {
        opcode: 0xA000,
        kind: InstructionKind::Bra { disp: 0 },
        flow: FlowKind::UnconditionalBranch,
    };
    assert!(inst.has_delay_slot());

    // ConditionalBranch does NOT have delay slot
    let inst = Instruction {
        opcode: 0x8900,
        kind: InstructionKind::Bt { disp: 0 },
        flow: FlowKind::ConditionalBranch,
    };
    assert!(!inst.has_delay_slot());

    // ConditionalBranchDelayed has delay slot
    let inst = Instruction {
        opcode: 0x8D00,
        kind: InstructionKind::Bts { disp: 0 },
        flow: FlowKind::ConditionalBranchDelayed,
    };
    assert!(inst.has_delay_slot());

    // Call has delay slot
    let inst = Instruction {
        opcode: 0xB000,
        kind: InstructionKind::Bsr { disp: 0 },
        flow: FlowKind::Call,
    };
    assert!(inst.has_delay_slot());

    // Return has delay slot
    let inst = Instruction {
        opcode: 0x000B,
        kind: InstructionKind::Rts,
        flow: FlowKind::Return,
    };
    assert!(inst.has_delay_slot());

    // ExceptionReturn has delay slot
    let inst = Instruction {
        opcode: 0x002B,
        kind: InstructionKind::Rte,
        flow: FlowKind::ExceptionReturn,
    };
    assert!(inst.has_delay_slot());

    // Normal does NOT have delay slot
    let inst = Instruction {
        opcode: 0x0009,
        kind: InstructionKind::Nop,
        flow: FlowKind::Normal,
    };
    assert!(!inst.has_delay_slot());
}

#[test]
fn test_branch_target_bt() {
    // BT with disp=3, PC=0x1000
    // target = 0x1000 + 4 + 3*2 = 0x100A
    let inst = Instruction {
        opcode: 0x8903,
        kind: InstructionKind::Bt { disp: 3 },
        flow: FlowKind::ConditionalBranch,
    };
    assert_eq!(inst.branch_target(0x1000), Some(0x100A));
}

#[test]
fn test_branch_target_bf_negative() {
    // BF with disp=-2 (0xFE as i8), PC=0x1000
    // target = 0x1000 + 4 + (-2)*2 = 0x1000
    let inst = Instruction {
        opcode: 0x8BFE,
        kind: InstructionKind::Bf { disp: -2 },
        flow: FlowKind::ConditionalBranch,
    };
    assert_eq!(inst.branch_target(0x1000), Some(0x1000));
}

#[test]
fn test_branch_target_bra() {
    // BRA with disp=0x100 (256), PC=0x2000
    // target = 0x2000 + 4 + 256*2 = 0x2204
    let inst = Instruction {
        opcode: 0xA100,
        kind: InstructionKind::Bra { disp: 0x100 },
        flow: FlowKind::UnconditionalBranch,
    };
    assert_eq!(inst.branch_target(0x2000), Some(0x2204));
}

#[test]
fn test_branch_target_bra_negative() {
    // BRA with disp=-1 (0xFFF sign-extended), PC=0x2000
    // target = 0x2000 + 4 + (-1)*2 = 0x2002
    let inst = Instruction {
        opcode: 0xAFFF,
        kind: InstructionKind::Bra { disp: -1 },
        flow: FlowKind::UnconditionalBranch,
    };
    assert_eq!(inst.branch_target(0x2000), Some(0x2002));
}

#[test]
fn test_branch_target_non_branch() {
    let inst = Instruction {
        opcode: 0x0009,
        kind: InstructionKind::Nop,
        flow: FlowKind::Normal,
    };
    assert_eq!(inst.branch_target(0x1000), None);
}

#[test]
fn test_literal_pool_addr_mov_l_pc_rel() {
    // MOV.L @(disp,PC), Rn with disp=3, PC=0x1000
    // addr = (0x1000 & 0xFFFFFFFC) + 4 + 3*4 = 0x1010
    let inst = Instruction {
        opcode: 0xD103,
        kind: InstructionKind::MovLPcRel {
            disp: 3,
            rn: Reg(1),
        },
        flow: FlowKind::Normal,
    };
    assert_eq!(inst.literal_pool_addr(0x1000), Some(0x1010));
}

#[test]
fn test_literal_pool_addr_mov_l_pc_rel_unaligned() {
    // MOV.L @(disp,PC), Rn with disp=3, PC=0x1002 (unaligned)
    // addr = (0x1002 & 0xFFFFFFFC) + 4 + 3*4 = 0x1000 + 4 + 12 = 0x1010
    let inst = Instruction {
        opcode: 0xD103,
        kind: InstructionKind::MovLPcRel {
            disp: 3,
            rn: Reg(1),
        },
        flow: FlowKind::Normal,
    };
    assert_eq!(inst.literal_pool_addr(0x1002), Some(0x1010));
}

#[test]
fn test_literal_pool_addr_mov_w_pc_rel() {
    // MOV.W @(disp,PC), Rn with disp=5, PC=0x1000
    // addr = 0x1000 + 4 + 5*2 = 0x100E
    let inst = Instruction {
        opcode: 0x9105,
        kind: InstructionKind::MovWPcRel {
            disp: 5,
            rn: Reg(1),
        },
        flow: FlowKind::Normal,
    };
    assert_eq!(inst.literal_pool_addr(0x1000), Some(0x100E));
}

#[test]
fn test_literal_pool_addr_mova() {
    // MOVA @(disp,PC), R0 with disp=10, PC=0x1002
    // addr = (0x1002 & 0xFFFFFFFC) + 4 + 10*4 = 0x1000 + 4 + 40 = 0x102C
    let inst = Instruction {
        opcode: 0xC70A,
        kind: InstructionKind::Mova { disp: 10 },
        flow: FlowKind::Normal,
    };
    assert_eq!(inst.literal_pool_addr(0x1002), Some(0x102C));
}

#[test]
fn test_literal_pool_addr_non_pc_rel() {
    let inst = Instruction {
        opcode: 0x0009,
        kind: InstructionKind::Nop,
        flow: FlowKind::Normal,
    };
    assert_eq!(inst.literal_pool_addr(0x1000), None);
}
