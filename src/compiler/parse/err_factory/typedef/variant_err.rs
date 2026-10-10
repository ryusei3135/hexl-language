use crate::compiler::{err, node};

use super::*;

/// これらの関数は呼び出し箇所が1〜2か所しかないので、inlineしても問題がない
impl Parser {
    /// variantキーワードが見つからなかった場合の例外
    #[inline(always)]
    pub(in crate::compiler::parse)
    fn variant_keyword_not_found(
        &self,
    ) -> Result<node::Group1Node, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::KeyWordVariant,
                context: lex::Tkn::KeyWordVariant
            }
        )
    }

    /// 共用体の名前がなかった
    #[inline(always)]
    pub(in crate::compiler::parse)
    fn variant_name_is_not_found(
        &self
    ) -> Result<node::Group1Node, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::Name("name".to_string()),
                context: lex::Tkn::KeyWordVariant
            }
        )
    }

    /// ```text
    /// variant a { // <- ここがない場合のエラー
    /// ```
    #[inline(always)]
    pub(in crate::compiler::parse) 
    fn variant_lbrace_not_found(
        &self,
        tkn: lex::Tkn,
    ) -> Result<(), err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: tkn,
                expected: lex::Tkn::LBrace,
                context: lex::Tkn::KeyWordVariant
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
    fn variant_in_unexpect_tkn(
        &self,
        expect: lex::Tkn,
    ) -> Result<(), err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: expect,
                context: lex::Tkn::KeyWordVariant
            }
        )
    }

    /// 共用体の追加オプションの機能で登録されていないもの
    /// が選択されたときのエラー
    /// ```
    /// variant"unsafe" name {}
    /// ```
    /// ## Args
    /// - option_name
    ///     - 所有権を渡す
    #[inline(always)]
    pub(in crate::compiler::parse)
    fn variant_option_unregister<T>(
        &self,
        option_name: String,
    ) -> Result<T, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::OptionIsNotFound { 
                found: option_name, 
                context: lex::Tkn::KeyWordVariant 
            }
        )
    }

    #[inline(always)]
    pub(in crate::compiler::parse)
    fn variant_option_next_name_not_found<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::Name("variant_name".to_string()),
                context: lex::Tkn::KeyWordVariant
            }
        )
    }
}