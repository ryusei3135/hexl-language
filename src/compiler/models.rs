use crate::asm_setting;
use crate::compiler::ast;

pub type Body = Vec<ast::Group2Info>;
pub type AsmFmtData = Option<asm_setting::AsmFormat>;
