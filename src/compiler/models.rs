use crate::asm_setting;
use crate::compiler::node;

pub type Body = Vec<node::Group2Info>;
pub type ReplaceVal = String;
pub type AsmFmtData = Option<asm_setting::AsmFormat>;
