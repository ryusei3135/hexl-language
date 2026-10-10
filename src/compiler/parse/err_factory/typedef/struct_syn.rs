use super::*;

impl Parser {
    pub(in crate::compiler::parse) 
    fn struct_name_is_not_found(
        &self,
    ) -> Result<node::Group1Node, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::Name("struct name".to_string()),
                context: lex::Tkn::KeyWordStruct
            }
        )
    }

    pub(in crate::compiler::parse) 
    fn struct_lbrace_not_found(
        &self,
        tkn: lex::Tkn,
    ) -> Result<(), err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: tkn,
                expected: lex::Tkn::LBrace,
                context: lex::Tkn::KeyWordStruct
            }
        )
    }

    pub(in crate::compiler::parse) 
    fn struct_in_unexpect_tkn(
        &self,
        expect: lex::Tkn,
    ) -> Result<(), err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: expect,
                context: lex::Tkn::KeyWordStruct
            }
        )
    }


    /// 構造体の初期化式の中に、メンバーの名前ではないトークンがあった
    pub(in crate::compiler::parse)
    fn struct_init_name_not_found<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found,
                expected: lex::Tkn::Name("struct init".to_string()),
                context: lex::Tkn::KeyWordStruct
            }
        )
    }

    /// 型の前の区切りが`:`ではなかった
    pub(in crate::compiler::parse)
    fn ty_colon_not_found<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::UnexpectedToken)
    }

    /// 構造体などの定義が`{`から始まっていなかった
    pub(in crate::compiler::parse)
    fn fields_lbrace_not_found<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::NotFoundTkn(Box::new(lex::Tkn::LBrace)))
    }

    /// メンバーの名前の次が、`:`でも`(`でもなかった
    pub(in crate::compiler::parse)
    fn field_colon_or_lparen_not_found<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::UnexpectedToken)
    }

    /// メンバーの型の後ろが、`,`でも`}`でも、次のメンバーの始まりでもなかった
    pub(in crate::compiler::parse)
    fn field_end_unexpected_tkn<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectTknAfterKeyword {
                keyword: lex::Tkn::Colon,
                expected: vec![",", "}"],
                found,
            }
        )
    }

    /// 型にジェネリクス(`Name<..>`)が使われたが、まだ対応していない
    pub(in crate::compiler::parse)
    fn generics_not_supported_yet<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::GenericsNotSupportedYet
        )
    }
}
