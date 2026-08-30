use super::*;


#[derive(Debug)]
pub enum LexErrKind {
    NumIsInvalid,
    FlagNotFound,
}

#[derive(Debug)]
pub struct LexErrs {
    pub span: Span,
    pub kind: LexErrKind,
}

#[macro_export]
macro_rules! lex_err {
    ($span:expr, $kind:ident) => {
        Err(crate::err::lex_err::LexErrs {
            span: $span,
            kind: crate::err::lex_err::LexErrKind::$kind
        })
    };
}
