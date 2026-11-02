pub mod mng_fmt;
pub(in crate::compiler::asm_gen) mod x64;
pub(in crate::compiler::asm_gen) mod aarch64;

use crate::compiler::ir::types::Size;
use crate::CpuKind;