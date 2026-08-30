use super::*;

pub(in crate::parse) struct MatchErr {
    span: err::Span,
    tkn: lex::Tkn,
}

impl MatchErr {
    pub fn new(span: err::Span, tkn: lex::Tkn) -> Self {
        Self { span, tkn }
    }

    /// スコープが`}`で閉じられているかを確認するAPI
    pub fn close_scope_to_rbrace(self, target: Option<lex::Tkn>) -> Result<(), err::ErrKind> {
        if !matches!(self.tkn, lex::Tkn::RBrace) {
            // match構文が`}`で閉じられていない
            crate::syntax_err!(self.span, err::SyntaxErrKind::UnenclosedScope { target })
        } else {
            Ok(())
        }
    }

    pub fn is_arrow_tkn(self) -> Result<(), err::ErrKind> {
        if !matches!(self.tkn, lex::Tkn::Arrow) {
            // 式の最後に`=>`(lex::Tkn::Arrow)がないので構文えらー
            crate::syntax_err!(
                self.span,
                err::SyntaxErrKind::UnexpectedTkn {
                    // 今のトークン
                    found: self.tkn,
                    // 期待したトークン
                    expected: lex::Tkn::Arrow,
                    context: lex::Tkn::KeyWordCond,
                }
            )
        } else {
            Ok(())
        }
    }
}

impl Parser {
    /// 条件分岐のエラーのバリアントを生成するAPIを提供する
    #[inline(always)]
    pub fn tkn_checker(&self) -> MatchErr {
        MatchErr::new(self.build_err_span(), self.current_tkn().clone())
    }
}
