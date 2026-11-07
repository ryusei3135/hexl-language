pub(in crate::compiler::asm_gen) mod aarch64;
pub mod mng_fmt;
mod static_fmt;
pub(in crate::compiler::asm_gen) mod x64;

use crate::CpuKind;
use crate::compiler::ir::types::Size;
