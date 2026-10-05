use crate::compiler::{err, node};

use super::*;

/// これらの関数は呼び出し箇所が1〜2か所しかないので、inlineしても問題がない
impl Parser {
    /// unionキーワードが見つからなかった場合の例外
    #[inline(always)]
    pub(in crate::compiler::parse)
    fn union_keyword_not_found(
        &self,
    ) -> Result<node::Group1Node, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::KeyWordUnion,
                context: lex::Tkn::KeyWordUnion
            }
        )
    }

    /// 共用体の名前がなかった
    #[inline(always)]
    pub(in crate::compiler::parse)
    fn union_name_is_not_found(
        &self
    ) -> Result<node::Group1Node, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::Name("name".to_string()),
                context: lex::Tkn::KeyWordUnion
            }
        )
    }

    /// ```text
    /// union a { // <- ここがない場合のエラー
    /// ```
    #[inline(always)]
    pub(in crate::compiler::parse) 
    fn union_lbrace_not_found(
        &self,
        tkn: lex::Tkn,
    ) -> Result<(), err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: tkn,
                expected: lex::Tkn::LBrace,
                context: lex::Tkn::KeyWordUnion
            }
        )
    }

    /// 共用体の初期化式の中に、予期しないトークンがあった
    /// ```text
    /// U { a 1 }      // <- `:`がない
    /// U { a: 1 : }   // <- 値の後ろに`:`がある
    /// ```
    #[inline(always)]
    pub(in crate::compiler::parse)
    fn union_in_unexpect_tkn(
        &self,
        expect: lex::Tkn,
    ) -> Result<(), err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: expect,
                context: lex::Tkn::KeyWordUnion
            }
        )
    }

    /// 共用体の追加オプションの機能で登録されていないもの
    /// が選択されたときのエラー
    /// ```
    /// union"unsafe" name {}
    /// ```
    /// ## Args
    /// - option_name
    ///     - 所有権を渡す
    #[inline(always)]
    pub(in crate::compiler::parse)
    fn union_option_unregister<T>(
        &self,
        option_name: String,
    ) -> Result<T, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::OptionIsNotFound { 
                found: option_name, 
                context: lex::Tkn::KeyWordUnion 
            }
        )
    }

    #[inline(always)]
    pub(in crate::compiler::parse)
    fn union_option_next_name_not_found<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::Name("union_name".to_string()),
                context: lex::Tkn::KeyWordUnion
            }
        )
    }
}