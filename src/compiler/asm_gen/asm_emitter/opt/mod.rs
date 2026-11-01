pub mod div_opt;
pub mod mul_opt;

use crate::compiler::asm_gen::AsmEmitter;
use crate::compiler::ir::inst::{self, Inst};
use crate::compiler::ir::types::Size;
use crate::err;
