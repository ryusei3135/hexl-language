use super::*;
use crate::compiler::ast::{ArgsNode, Expr};

impl Parser {
    pub(in crate::compiler::hexl_frontend::parse) fn fn_unexpect_tkn<R>(&self) -> Result<R, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::LBrace,
                context: lex::Tkn::KeyWordStruct
            },
            Rc::clone(&self.file_name)
        )
    }

    pub(in crate::compiler::hexl_frontend::parse) fn not_found_lparen(&self) -> Result<Expr, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "(",
                found: self.current_tkn().clone(),
            },
            Rc::clone(&self.file_name)
        )
    }

    pub(in crate::compiler::hexl_frontend::parse) fn args_expr_in_unexpect_tkn(
        &self,
        tkn: lex::Tkn,
    ) -> Result<Vec<ArgsNode>, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "name or `)`",
                found: tkn,
            },
            Rc::clone(&self.file_name)
        )
    }

    /// 未実装の機能が使われた(メゾットのジェネリクスなど)
    pub(in crate::compiler::hexl_frontend::parse) fn fn_not_implemented<T>(
        &self,
        feature: &'static str,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::NotImplemented { feature },
            Rc::clone(&self.file_name)
        )
    }

    /// 関数名の次が`(`でも`<`でもなかった
    pub(in crate::compiler::hexl_frontend::parse) fn fn_lparen_or_langle_not_found<T>(
        &self,
        found: lex::Tkn,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "( or <",
                found,
            },
            Rc::clone(&self.file_name)
        )
    }

    /// 引数の定義の後ろが`{`ではなかった(戻り値の型の後ろなど)
    pub(in crate::compiler::hexl_frontend::parse) fn fn_lbrace_not_found<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::NotFoundTkn(Box::new(lex::Tkn::LBrace)))
    }

    /// 引数の定義が`(`から始まっていなかった
    pub(in crate::compiler::hexl_frontend::parse) fn fn_args_lparen_not_found<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::NotFoundTkn(Box::new(lex::Tkn::LParen)))
    }

    /// 引数リストの後ろが、`:`でも`{`でもなかった
    pub(in crate::compiler::hexl_frontend::parse) fn fn_colon_or_lbrace_not_found<T>(
        &self,
        found: lex::Tkn,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: ": or `{`",
                found,
            },
            Rc::clone(&self.file_name)
        )
    }

    /// `,`の前に引数が無い(先頭が`,`、または`,,`)
    pub(in crate::compiler::hexl_frontend::parse) fn fn_arg_before_comma_not_found<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "name",
                found: lex::Tkn::Comma,
            },
            Rc::clone(&self.file_name)
        )
    }

    /// 引数の後ろが、`,`でも`)`でもなかった
    pub(in crate::compiler::hexl_frontend::parse) fn fn_comma_or_rparen_not_found<T>(
        &self,
        found: lex::Tkn,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: ", or `)`",
                found,
            },
            Rc::clone(&self.file_name)
        )
    }

    /// ジェネリクス関数の定義(`name<T, U>(..) { .. }`)で、
    /// `tkns[idx]`が期待したトークンではなかった(エラーの位置は`tkns[idx]`)
    pub(in crate::compiler::hexl_frontend::parse) fn generic_def_expected<T>(
        &self,
        idx: usize,
        expected: &'static str,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        // トークンが無い/`idx`が範囲外の場合は、最後のトークンの位置にする
        let Some(t) = self
            .tkns
            .as_ref()
            .and_then(|tkns| tkns.get(idx).or(tkns.last()))
        else {
            return self.tkn_is_eof(Vec::new());
        };
        crate::syntax_err!(
            Rc::clone(&t.span),
            err::SyntaxErrKind::ExpectedKind {
                expected,
                found: t.tkn.clone(),
            },
            Rc::clone(&self.file_name)
        )
    }

    /// ジェネリクス関数の定義の途中でトークンが終わった(エラーの位置は最後のトークン)
    pub(in crate::compiler::hexl_frontend::parse) fn generic_def_eof<T>(
        &self,
        expected: Vec<&'static str>,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        let Some(t) = self.tkns.as_ref().and_then(|tkns| tkns.last()) else {
            return self.tkn_is_eof(Vec::new());
        };
        crate::syntax_err!(
            Rc::clone(&t.span),
            err::SyntaxErrKind::TknIsEof { expected },
            Rc::clone(&self.file_name)
        )
    }

    /// ジェネリクス関数の型引数が多すぎる(`,`が余分)
    pub(in crate::compiler::hexl_frontend::parse) fn generic_type_args_too_many<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "`>`",
                found: lex::Tkn::Comma,
            },
            Rc::clone(&self.file_name)
        )
    }

    /// ジェネリクス関数の型引数が足りない(`>`が早すぎる)
    pub(in crate::compiler::hexl_frontend::parse) fn generic_type_args_too_few<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "`,`",
                found: lex::Tkn::RAngleBracket,
            },
            Rc::clone(&self.file_name)
        )
    }

    /// ジェネリクス関数の型引数の後ろが、`,`でも`>`でもなかった
    pub(in crate::compiler::hexl_frontend::parse) fn generic_type_args_comma_or_rangle_not_found<T>(
        &self,
        found: lex::Tkn,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: ", or `>`",
                found,
            },
            Rc::clone(&self.file_name)
        )
    }
}
