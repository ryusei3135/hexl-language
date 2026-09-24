use super::*;

impl Parser {
    pub(in crate::parse) fn struct_name_is_not_found(
        &self,
    ) -> Result<node::Group1Node, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::Name("struct name".to_string()),
                context: lex::Tkn::KeyWordStruct
            }
        )
    }

    pub(in crate::parse) fn struct_lbrace_not_found(
        &self,
        tkn: lex::Tkn,
    ) -> Result<(), err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: tkn,
                expected: lex::Tkn::LBrace,
                context: lex::Tkn::KeyWordStruct
            }
        )
    }

    pub(in crate::parse) fn struct_in_unexpect_tkn(
        &self,
        expect: lex::Tkn,
    ) -> Result<(), err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: expect,
                context: lex::Tkn::KeyWordStruct
            }
        )
    }
}
