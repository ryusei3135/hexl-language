use super::*;

pub(in crate::compiler::parse) struct MatchErr {
    span: err::Span,
    tkn: lex::Tkn,
    file_name: Rc<String>,
}

impl MatchErr {
    #[inline(always)]
    pub fn new(span: err::Span, tkn: lex::Tkn, file_name: Rc<String>) -> Self {
        Self { span, tkn, file_name }
    }

    /// スコープが`}`で閉じられているかを確認するAPI
    pub fn close_scope_to_rbrace(self, target: Option<lex::Tkn>) -> Result<(), err::ErrKind> {
        if !matches!(self.tkn, lex::Tkn::RBrace) {
            // match構文が`}`で閉じられていない
            crate::err_generated_by!();
            crate::syntax_err!(
                self.span, 
                err::SyntaxErrKind::UnenclosedScope { target },
                Rc::clone(&self.file_name)
            )
        } else {
            Ok(())
        }
    }

    pub fn is_arrow_tkn(self) -> Result<(), err::ErrKind> {
        if !matches!(self.tkn, lex::Tkn::Arrow) {
            // 式の最後に`=>`(lex::Tkn::Arrow)がないので構文えらー
            crate::err_generated_by!();
            crate::syntax_err!(
                self.span,
                err::SyntaxErrKind::UnexpectedTkn {
                    // 今のトークン
                    found: self.tkn,
                    // 期待したトークン
                    expected: lex::Tkn::Arrow,
                    context: lex::Tkn::KeyWordCond,
                },
                Rc::clone(&self.file_name)
            )
        } else {
            Ok(())
        }
    }
}

impl Parser {
    /// 条件分岐のエラーのバリアントを生成するAPIを提供する
    #[inline(always)]
    pub(in crate::compiler::parse) fn tkn_checker(&self) -> MatchErr {
        MatchErr::new(self.build_err_span(), self.current_tkn().clone(), Rc::clone(&self.file_name))
    }

    pub(in crate::compiler::parse) fn cond_keyword_not_found(
        &self,
    ) -> Result<node::Expr, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::UnexpectedTkn {
                found: self.current_tkn().clone(),
                expected: lex::Tkn::KeyWordCond,
                context: lex::Tkn::KeyWordCond
            },
            Rc::clone(&self.file_name)
        )
    }

    /// `match`の`{`が無かった
    pub(in crate::compiler::parse) fn cond_expr_scope_start_lbrace<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::cond_err!(
            self.build_err_span(), 
            CondExprScopeStartLBrace,
            Rc::clone(&self.file_name)
        )
    }

    /// アームの`=>`の次に`{`が無かった
    pub(in crate::compiler::parse) fn cond_expr_pattern_lbrace<T>(
        &self,
    ) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::cond_err!(
            self.build_err_span(), 
            CondExprPatternLBrace,
            Rc::clone(&self.file_name)
        )
    }

    /// `else =>`の次に`{`が無かった
    pub(in crate::compiler::parse) fn cond_else_lbrace<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::cond_err!(
            self.build_err_span(), 
            CondElseLBrace,
            Rc::clone(&self.file_name)
        )
    }

    /// `else`が無かった
    pub(in crate::compiler::parse) fn cond_else_not_found<T>(&self) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::cond_err!(
            self.build_err_span(), 
            CondElseNotFound,
            Rc::clone(&self.file_name)
        )
    }
}
