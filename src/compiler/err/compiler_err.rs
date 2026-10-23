//! 構文ではなくこのhexlコンパイラ自身のエラー

use crate::compiler::ir::types;

#[derive(Debug, Clone, PartialEq)]
pub enum CompilerErr {
    ThisTyIsCantConvertRegsiter(types::Size),
}
