//! 構文ではなくこのhexlコンパイラ自身のエラー

use crate::compiler::ir::types;

#[derive(Debug, Clone, PartialEq)]
pub enum CompilerErrKind {
    ThisTyIsCantConvertRegsiter(types::Size),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompilerErr {
    kind: CompilerErrKind,
}

#[macro_export]
macro_rules! GenCompilerErr {
    ($kind:expr) => {
        Err(err::ErrKind::CompilerErr(Box::new(CompilerErr {
            kind: $kind,
        })))
    };
}