//! 構文ではなくこのhexlコンパイラ自身のエラー

use super::{CAT_INTERNAL, Span, span_loc, write_err};
use crate::compiler::ir::types;
use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum CompilerErrKind {
    ThisTyIsCantConvertRegsiter(types::Size),
}

impl fmt::Display for CompilerErrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThisTyIsCantConvertRegsiter(size) => {
                write!(f, "サイズ`{:?}`の型はレジスタに変換できません", size)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompilerErr {
    pub kind: CompilerErrKind,
    /// エラーの発生位置。分からない(コンパイラ内部で起きた)ときは`None`
    pub span: Option<Rc<Span>>,
}

impl fmt::Display for CompilerErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_err(
            f,
            CAT_INTERNAL,
            &self.kind,
            span_loc(self.span.as_ref()).map(|s| s as &dyn fmt::Display),
        )
    }
}

/// `GenCompilerErr!(kind)` / `GenCompilerErr!(kind, span)`
#[macro_export]
macro_rules! GenCompilerErr {
    ($kind:expr) => {
        Err(err::ErrKind::CompilerErr(Box::new(CompilerErr {
            kind: $kind,
            span: None,
        })))
    };
    ($kind:expr, $span:expr) => {
        Err(err::ErrKind::CompilerErr(Box::new(CompilerErr {
            kind: $kind,
            span: Some($span),
        })))
    };
}
