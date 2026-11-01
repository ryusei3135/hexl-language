pub mod expr_ir;
pub mod mem_ir;
pub mod misc_ir;
pub mod syntax;

use super::*;
use crate::compiler::ast::{Group2Node, MatchArm, StmtNode};
use crate::compiler::optimization::x64::gen_ir::constant_fold::fold_expr;
