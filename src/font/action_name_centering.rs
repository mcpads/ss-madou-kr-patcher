//! Center action names using the last non-space word, without changing storage.
//!
//! The shared action-name UI reads skill records and, when actor+0x70 == 1,
//! the item table. This does not patch general dialogue/menu rendering.
//! R5 counts every word before the first FFxx;
//! R2 remembers that count only after a non-00B2 word. Thus internal spaces
//! retain their width while trailing spaces do not affect the starting column.
//! Intentional trailing spaces and fixed-slot padding are indistinguishable at
//! runtime: both are excluded by this UI's centering policy. Text, terminators,
//! references, drawing, and action effects are left intact.
use anyhow::{Result, ensure};
use sh2_asm::{Assembler, Instruction as I, Register as R, decode_word, encode_word};

const OFFSET: usize = 0x105bc;
const ORIGIN: u32 = 0x060145bc;
const ORIGINAL: [u8; 44] = [
    0x61, 0x63, 0x71, 0x7c, 0x63, 0x12, 0x61, 0x31, 0xd7, 0x14, 0x61, 0x1d, 0x31, 0x76, 0x8d, 0x08,
    0xe2, 0x00, 0xe5, 0x00, 0x75, 0x02, 0x60, 0x53, 0x01, 0x3d, 0x61, 0x1d, 0x31, 0x76, 0x8f, 0xf9,
    0x72, 0x01, 0x76, 0x74, 0x61, 0x2b, 0x71, 0x16, 0x41, 0x18, 0x26, 0x11,
];
// Verified Japanese file 0 bytes. These branches enter the replacement at
// 060145BE (with R1=R6 in the delay slot) and 060145BC respectively.
const GUARDS: &[(usize, &[u8])] = &[
    (OFFSET, &ORIGINAL),
    (0x1057a, &[0xa0, 0x20, 0x61, 0x63]),
    (0x1059e, &[0xa0, 0x0d, 0x23, 0x12]),
    (0x10618, &[0x00, 0x00, 0xfe, 0xff]),
];

fn replacement() -> Result<Vec<u8>> {
    let instructions = [
        // Preserve both inbound entry paths, including the R1=R6 delay slot.
        I::MovReg {
            rm: R::R6,
            rn: R::R1,
        },
        I::AddImm {
            imm: 124,
            rn: R::R1,
        },
        I::MovLLoad {
            rm: R::R1,
            rn: R::R3,
        },
        // At 060145C2, ((PC+4)&!3) + 0x15*4 = 06014618.
        I::MovLPcRel {
            disp: 0x15,
            rn: R::R7,
        },
        I::MovImm {
            imm: -78,
            rn: R::R4,
        },
        I::ExtuB {
            rm: R::R4,
            rn: R::R4,
        },
        I::MovImm { imm: 0, rn: R::R2 },
        I::MovImm { imm: 0, rn: R::R5 },
        I::MovWLoadPostInc {
            rm: R::R3,
            rn: R::R1,
        }, // 060145CC
        I::ExtuW {
            rm: R::R1,
            rn: R::R1,
        },
        I::CmpHi {
            rm: R::R7,
            rn: R::R1,
        },
        I::Bt { disp: 4 }, // FFxx -> 060145DE, before counting/loading again
        I::AddImm { imm: 1, rn: R::R5 },
        I::CmpEq {
            rm: R::R4,
            rn: R::R1,
        },
        I::Bt { disp: -8 },  // space -> 060145CC, retain last visible count
        I::Bra { disp: -9 }, // non-space -> 060145CC
        I::MovReg {
            rm: R::R5,
            rn: R::R2,
        }, // BRA delay slot
        // Original (22-count)<<8 calculation and write to UI object+0x74.
        I::AddImm {
            imm: 116,
            rn: R::R6,
        },
        I::Neg {
            rm: R::R2,
            rn: R::R1,
        },
        I::AddImm { imm: 22, rn: R::R1 },
        I::Shll8 { rn: R::R1 },
        I::MovWStore {
            rm: R::R1,
            rn: R::R6,
        },
    ];
    let mut assembler = Assembler::new();
    assembler.emit_all(instructions.clone());
    let program = assembler.assemble(ORIGIN)?;
    let bytes = program.bytes().to_vec();
    ensure!(
        bytes.len() == ORIGINAL.len(),
        "action-name patch exceeds original code span"
    );
    for (word, instruction) in bytes.chunks_exact(2).zip(instructions) {
        let encoded = u16::from_be_bytes([word[0], word[1]]);
        ensure!(
            decode_word(encoded)? == instruction && encode_word(&instruction)? == encoded,
            "action-name instruction roundtrip differs"
        );
    }
    ensure!(
        bytes[..6] == ORIGINAL[..6],
        "inbound entry instructions changed"
    );
    Ok(bytes)
}

/// Patch the known file 0 routine, rejecting incompatible or already patched
/// input before any mutation. The combined first-read writer applies this
/// before its final disc write.
pub fn patch(first_read: &mut [u8]) -> Result<()> {
    for &(offset, expected) in GUARDS {
        ensure!(
            first_read.get(offset..offset + expected.len()) == Some(expected),
            "action-name centering prerequisite differs at file 0+{offset:#x}"
        );
    }
    let bytes = replacement()?;
    first_read[OFFSET..OFFSET + bytes.len()].copy_from_slice(&bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supported_input() -> Vec<u8> {
        let mut input = vec![0x5a; 0x10620];
        for &(offset, expected) in GUARDS {
            input[offset..offset + expected.len()].copy_from_slice(expected);
        }
        input
    }

    // Requirement: a supported file changes only the known centering routine;
    // other entry instructions, the terminator literal and all storage survive.
    #[test]
    fn changes_only_the_centering_routine() {
        let original = supported_input();
        let mut patched = original.clone();
        patch(&mut patched).unwrap();
        assert_ne!(&patched[OFFSET..OFFSET + ORIGINAL.len()], &ORIGINAL);
        assert_eq!(&patched[..OFFSET + 6], &original[..OFFSET + 6]);
        assert_eq!(
            &patched[OFFSET + ORIGINAL.len()..],
            &original[OFFSET + ORIGINAL.len()..]
        );
        // A second application must fail without modifying the patched file.
        let before_retry = patched.clone();
        assert!(patch(&mut patched).is_err());
        assert_eq!(patched, before_retry);
    }

    // Requirement: stale code, either inbound branch/delay slot or the FEFF
    // literal must not produce a partially patched executable.
    #[test]
    fn rejects_each_incompatible_prerequisite_without_writing() {
        for &(offset, expected) in GUARDS {
            for delta in 0..expected.len() {
                let mut input = supported_input();
                input[offset + delta] ^= 1;
                let unchanged = input.clone();
                assert!(
                    patch(&mut input).is_err(),
                    "accepted mismatch at {:#x}",
                    offset + delta
                );
                assert_eq!(input, unchanged);
            }
        }
    }

    #[test]
    fn rejects_truncated_input_without_writing() {
        let mut input = supported_input();
        input.truncate(0x1061b);
        let unchanged = input.clone();
        assert!(patch(&mut input).is_err());
        assert_eq!(input, unchanged);
    }
}
