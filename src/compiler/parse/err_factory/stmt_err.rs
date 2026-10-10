//! 文(`stmt.rs`)で発生するエラー

use super::*;

impl Parser {
    /// 文の先頭に、予期しないトークンがあった
    pub(in crate::compiler::parse)
    fn unexpect_tkn_in_stmt<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectTknInStmt { found }
        )
    }

    /// `[`の次が名前ではなかった
    pub(in crate::compiler::parse)
    fn stmt_name_not_found_after_lbracket<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "name",
                found,
            }
        )
    }

    /// 反復処理の外で、`continue`/`break`が使われた
    pub(in crate::compiler::parse)
    fn loop_control_outside_loop<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        Err(err::ErrKind::Syntax(Box::new(err::SyntaxErr {
            kind: err::SyntaxErrKind::UnexpectTknInStmt {
                found: self.current_tkn().clone(),
            },
            loc: err::ErrLoc::new(
                self.build_err_span(),
                "parse::stmt::loop_control_node".to_string(),
            ),
        })))
    }

    /// `pub`の次が、関数の名前ではなかった
    pub(in crate::compiler::parse)
    fn unexpect_tkn_after_pub<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectTknAfterKeyword {
                keyword: lex::Tkn::KeyWordPub,
                expected: vec!["struct", "variant", "name"],
                found,
            }
        )
    }

    /// 反復処理の条件の後に`{`が無かった
    pub(in crate::compiler::parse)
    fn loop_lbrace_not_found<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "{",
                found: self.current_tkn().clone(),
            }
        )
    }

    /// `#`の次が名前ではなかった
    pub(in crate::compiler::parse)
    fn preproc_name_not_found<T>(&self, found: lex::Tkn) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::ExpectedKind {
                expected: "name",
                found,
            }
        )
    }
}
