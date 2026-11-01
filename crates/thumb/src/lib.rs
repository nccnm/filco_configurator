//! ARMv6-M (Cortex-M0) Thumb instruction set: the subset a Cortex-M0 runs, nothing from Thumb-2 except the
//! 32-bit `bl`, `msr`, `mrs` and barriers.
//!
//! - [`asm`] assembles the source text of the firmware patches (tap-hold and the info command). It encodes
//!   exactly as Keystone (LLVM MC) does for that source, so a patched image comes out byte for byte the same as
//!   the images built and flashed by the analysis repository. It never picks a 32-bit encoding other than `bl`:
//!   a conditional branch out of range is an error, not a silent Thumb-2 branch.
//! - [`decode`] turns halfwords into [`decode::Op`]; the emulator (`m0emu`) and the static stack analysis use it.
#![forbid(unsafe_code)]

pub mod asm;
pub mod decode;

pub use asm::{AsmError, Program, assemble, bl};
pub use decode::{AluOp, Cond, Insn, MemOp, Op, decode};
