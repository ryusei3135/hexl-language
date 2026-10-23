use super::*;

impl Parser {
    /// 最初にキーワードのcondが来る必要がある
    pub(in crate::compiler::hexl_frontend::parse) fn expr_match(
        &mut self,
    ) -> Result<Expr, err::ErrKind> {
        const STRUCT_NOT_INIT: bool = false;

        if self.current_tkn() != &lex::Tkn::KeyWordCond {
            return crate::err_at!(self.cond_keyword_not_found());
        }
        // `cond`の位置
        let span = self.build_err_span();
        // match の対象式
        let cond_expr = if self.peek_tkn()? == lex::Tkn::LBrace {
            self.next_tkn(&[])?;
            None
        } else if self.is_variant_bind_ahead() {
            // バリアントのパターン束縛: `cond M(val) := a { .. } | { .. }`
            // 真偽値として扱われるので、下の`is_bool_expr`でif/elseになる
            Some(Box::new(self.expr_variant_bind()?))
        } else {
            // 構造体を初期化する式を代入することはできないので、`false`
            Some(Box::new(self.expr_or(STRUCT_NOT_INIT)?))
        };

        // 真偽値(比較式)が与えられた場合は、単純なif/elseとして扱う
        // `cond a == 10 { .. } else { .. }`
        if cond_expr.as_ref().is_some_and(|e| Self::is_bool_expr(e)) {
            if let Some(cond) = cond_expr {
                return self.expr_match_bool(*cond, Rc::clone(&span));
            }
        }

        // {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return crate::err_at!(self.cond_expr_scope_start_lbrace());
        }

        let mut arms = Vec::new();

        loop {
            // }
            //if matches!(self.current_tkn(), lex::Tkn::RBrace) {
            //    break;
            //}
            // elseのノードを作成する
            if self.peek_tkn()? == lex::Tkn::KeyWordElse {
                self.next_tkn(&[])?;
                return self.build_else_arm_node(&cond_expr, arms, span);
            }
            // if
            let pattern = if self.is_variant_bind_ahead() {
                // 対象の式がない`cond { .. }`のアームの条件式なら、
                // パターン束縛を使える: `M(val) := a => { .. }`
                // (`cond a { .. }`のアームは`a`との値の比較なので使えない)
                if cond_expr.is_some() {
                    self.next_tkn(&[])?;
                    return crate::err_at!(self.variant_bind_outside_cond());
                }
                self.expr_variant_bind()?
            } else {
                self.expr_or(STRUCT_NOT_INIT)?
            };

            // => 構文エラーを返す
            // 式の最後に`=>`(lex::Tkn::Arrow)がないので構文えらー
            crate::err_at!(self.tkn_checker()).is_arrow_tkn()?;
            // {
            if self.next_tkn(&["{"])? != lex::Tkn::LBrace {
                return crate::err_at!(self.cond_expr_pattern_lbrace());
            }
            self.next_tkn(&[])?;
            // ここでアーム本体を解析
            let body = self.gen_block_node()?;
            // }
            crate::err_at!(self.tkn_checker()).close_scope_to_rbrace(None)?;
            if matches!(self.next_tkn_ref(&[])?, lex::Tkn::RBrace) {
                break;
            }
            // アームの位置は、パターンの位置
            let arm_span = pattern.span.clone();
            arms.push(MatchArm {
                pattern: Box::new(pattern),
                body,
                span: arm_span,
            });
        }

        // 式の終了
        self.next_tkn(&["}"])?; //}

        let node = Expr::new(
            ExprNode::Match {
                pattern: cond_expr,
                arms,
                arm_else: None,
            },
            span,
        );
        Ok(node)
    }

    /// 条件分岐のelseのノードやbodyを処理する
    fn build_else_arm_node(
        &mut self,
        cond_expr: &Option<Box<Expr>>,
        cond_arms: Vec<MatchArm>,
        span: std::rc::Rc<err::Span>,
    ) -> Result<Expr, err::ErrKind> {
        // =>
        // 式の最後に`=>`(lex::Tkn::Arrow)がないので構文えらー
        self.next_tkn(&[])?;
        crate::err_at!(self.tkn_checker()).is_arrow_tkn()?;
        // {
        if !matches!(self.next_tkn(&["{"])?, lex::Tkn::LBrace) {
            return crate::err_at!(self.cond_else_lbrace());
        }
        self.next_tkn(&[])?;
        // ここでアーム本体を解析
        let body = self.gen_block_node()?;
        // }
        self.next_tkn(&[])?;
        crate::err_at!(self.tkn_checker()).close_scope_to_rbrace(None)?;
        // 条件分岐を閉じる`}`
        crate::err_at!(self.tkn_checker()).close_scope_to_rbrace(Some(lex::Tkn::KeyWordCond))?;
        //}
        self.next_tkn(&[])?;
        let node = Expr::new(
            ExprNode::Match {
                pattern: cond_expr.clone(),
                arms: cond_arms,
                arm_else: Some(body),
            },
            span,
        );
        return Ok(node);
    }

    /// 与えられた式が真偽値を返す(比較)式かどうか
    /// (バリアントのパターン束縛`M(val) := a`も、`a`が`M`かどうかの真偽値)
    ///
    /// `match` に渡された対象の式がこれに該当する場合、
    /// 「真偽値を与える」形式(単純なif/else)として扱う
    fn is_bool_expr(expr: &Expr) -> bool {
        matches!(
            expr.node,
            ExprNode::LessThen(_)
                | ExprNode::GreaterThen(_)
                | ExprNode::Equal(_)
                | ExprNode::NotEq(_)
                | ExprNode::VariantBind(_)
        )
    }

    /// `match` に真偽値(比較式)が渡された場合の解析
    /// ```
    /// cond a == 10 {
    ///     // a が 10 のとき
    /// } else {
    ///     // それ以外 (`else`は省略可能)
    /// }
    /// ```
    /// これは `if` / `else` と同じ意味を持つ
    fn expr_match_bool(
        &mut self,
        cond: Expr,
        span: std::rc::Rc<err::Span>,
    ) -> Result<Expr, err::ErrKind> {
        // match expr {
        // {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return crate::err_at!(self.cond_expr_scope_start_lbrace());
        }
        self.next_tkn(&[])?;
        // trueのときの処理
        let body = self.gen_block_node()?;
        // }
        crate::err_at!(self.tkn_checker()).close_scope_to_rbrace(None)?;

        // `else`は省略できる: `cond a == 10 { .. }`
        // 次のトークンが`else`のときだけ、else節を解析する
        let else_body = if matches!(self.next_tkn_ref(&[]), Ok(lex::Tkn::KeyWordElse)) {
            // else
            self.next_tkn(&["else"])?;
            // {
            if !matches!(self.next_tkn(&["{"])?, lex::Tkn::LBrace) {
                return crate::err_at!(self.cond_else_lbrace());
            }
            self.next_tkn(&[])?;
            // それ以外のときの処理
            let else_body = self.gen_block_node()?;
            // }
            crate::err_at!(self.tkn_checker()).close_scope_to_rbrace(None)?;
            Some(else_body)
        } else {
            None
        };
        // 式の終了
        self.next_tkn(&[])?;

        // アームの位置は、条件式の位置
        let arm_span = cond.span.clone();
        Ok(Expr::new(
            ExprNode::Match {
                pattern: None,
                arms: vec![MatchArm {
                    pattern: Box::new(cond),
                    body,
                    span: arm_span,
                }],
                arm_else: else_body,
            },
            span,
        ))
    }
}
