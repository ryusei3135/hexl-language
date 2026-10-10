//! 契約(`must`/`of`、`contract.rs`)で発生するエラー

use super::*;

impl Parser {
    /// `must`の次が`=`ではなかった
    pub(in crate::compiler::parse)
    fn missing_equals_after_must<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::MissingEqualsAfterMust { found }
        )
    }

    /// `must=`の次のトークンが無かった(EOF)
    pub(in crate::compiler::parse)
    fn must_name_is_eof<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::TknIsEof {
                expected: vec!["Name"]
            }
        )
    }

    /// `of`の次が名前ではなかった
    pub(in crate::compiler::parse)
    fn missing_ident_after_of<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::MissingIdentAfterOf { found }
        )
    }
}
