pub mod mul_opt;
pub mod div_opt;

use crate::compiler::ir::types::Size;
use crate::err;
use crate::compiler::asm_gen::AsmEmitter;
use crate::compiler::ir::inst::{self, Inst};