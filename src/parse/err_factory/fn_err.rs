use super::*;


impl Parser {
    pub(in crate::parse)
    fn fn_unexpect_tkn<R>(
        &self
    ) -> Result<R, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::LBrace,
                context: lex::Tkn::KeyWordStruct
            }
        )
    }

    pub(in crate::parse)
    fn not_found_lparen(
        &self
    ) -> Result<node::Expr, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "(",
                found: self.current_tkn().clone(),
            }
        )
    }

    pub(in crate::parse)
    fn args_expr_in_unexpect_tkn(
        &self,
        tkn: lex::Tkn,
    ) -> Result<Vec<node::ArgsNode>, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "name or `)`",
                found: tkn,
            }
        )
    }
}