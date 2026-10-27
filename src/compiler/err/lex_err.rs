use super::*;

use std::fmt;

#[derive(Debug)]
pub enum LexErrKind {
    NumIsInvalid,
    FlagNotFound,
}

impl fmt::Display for LexErrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::NumIsInvalid => "数値の形式が不正です",
            Self::FlagNotFound => "フラグが見つかりません",
        };
        write!(f, "{}", msg)
    }
}

#[derive(Debug)]
pub struct LexErrs {
    pub span: Rc<Span>,
    pub kind: LexErrKind,
}

impl fmt::Display for LexErrs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_err(
            f,
            CAT_LEX,
            &self.kind,
            span_loc(Some(&self.span)).map(|s| s as &dyn fmt::Display),
        )
    }
}

impl std::error::Error for LexErrs {}

#[macro_export]
macro_rules! lex_err {
    ($span:expr, $kind:ident) => {
        Err(crate::compiler::err::lex_err::LexErrs {
            span: $span,
            kind: crate::compiler::err::lex_err::LexErrKind::$kind,
        })
    };
}
