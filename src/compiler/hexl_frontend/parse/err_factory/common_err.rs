//! 特定の構文に限らない、汎用的なエラー
//!
//! 以前は`panic!`/`unwrap`/`expect`で止めていた箇所を、
//! `Err`で返すために使う。

use super::*;

impl Parser {
    /// `expected`のトークンが来るはずの位置に、`found`があった
    ///
    /// ## Args
    /// - expected 期待していたトークンの説明
    /// - found 実際にあったトークン
    pub(in crate::compiler::hexl_frontend::parse) fn expected_tkn<T>(
        &self,
        expected: &'static str,
        found: lex::Tkn,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind { expected, found },
            Rc::clone(&self.file_name)
        )
    }

    /// `expected`のトークンが来るはずの位置に、現在のトークンがあった
    pub(in crate::compiler::hexl_frontend::parse) fn expected_at_current<T>(
        &self,
        expected: &'static str,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected,
                found: self.current_tkn().clone(),
            },
            Rc::clone(&self.file_name)
        )
    }

    /// 数値のトークンが、数値として読み取れなかった(桁あふれなど)
    pub(in crate::compiler::hexl_frontend::parse) fn invalid_number<T>(
        &self,
        num: String,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "number",
                found: lex::Tkn::Number(num),
            },
            Rc::clone(&self.file_name)
        )
    }

    /// 構造体/バリアントの外で`Self`が使われた
    pub(in crate::compiler::hexl_frontend::parse) fn self_outside_of_type<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectTknInExpr {
                found: lex::Tkn::KeyWordSelf,
            },
            Rc::clone(&self.file_name)
        )
    }

    /// パーサーの内部状態が想定外だった(本来は起こらない)
    ///
    /// ## Args
    /// - what 何が想定外だったか(エラーの`expected`に入る)
    pub(in crate::compiler::hexl_frontend::parse) fn internal_state_err<T>(
        &self,
        what: &'static str,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: what,
                found: self.current_tkn().clone(),
            },
            Rc::clone(&self.file_name)
        )
    }
}
