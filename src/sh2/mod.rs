//! SH-2 instruction decoder for Sega Saturn disassembly.
//!
//! This module provides:
//!
//! - [`Instruction`] -- a decoded SH-2 instruction with opcode, kind, and flow information.
//! - [`InstructionKind`] -- enum covering the full SH-2 instruction set.
//! - [`Reg`] -- register operand (R0-R15).
//! - [`FlowKind`] -- control-flow classification of instructions.
//! - [`decode`] -- the main decoder: converts a 16-bit opcode into an `Instruction`.

pub mod decode;
mod display;
pub mod instruction;

pub use decode::decode;
pub use instruction::{FlowKind, Instruction, InstructionKind, Reg};
