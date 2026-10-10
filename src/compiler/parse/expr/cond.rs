use super::*;

impl Parser {
    /// 最初にキーワードのcondが来る必要がある
    pub(in crate::compiler::parse) 
    fn expr_match(&mut self) -> Result<node::Expr, err::ErrKind> {
        const STRUCT_NOT_INIT: bool = false;

        if self.current_tkn() != &lex::Tkn::KeyWordCond {
            return crate::err_at!(self.cond_keyword_not_found());
        }
        // match の対象式
        let cond_expr = if self.peek_tkn().unwrap() == lex::Tkn::LBrace {
            self.next_tkn(&[])?;
            None
        } else {
            // 構造体を初期化する式を代入することはできないので、`false`
            Some(Box::new(self.expr_cmp(STRUCT_NOT_INIT)?))
        };

        // 真偽値(比較式)が与えられた場合は、単純なif/elseとして扱う
        // `cond a == 10 { .. } else { .. }`
        if let Some(ref target_expr) = cond_expr {
            if Self::is_bool_expr(target_expr) {
                let cond = *cond_expr.unwrap();
                return self.expr_match_bool(cond);
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
                return self.build_else_arm_node(&cond_expr, arms);
            }
            // if
            let pattern = self.expr_cmp(STRUCT_NOT_INIT)?;

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
            arms.push(node::MatchArm {
                pattern: Box::new(pattern),
                body,
            });
        }

        // 式の終了
        self.next_tkn(&["}"])?; //}

        let node = node::Expr::Match {
            pattern: cond_expr,
            arms,
            arm_else: None,
        };
        Ok(node)
    }

    /// 条件分岐のelseのノードやbodyを処理する
    fn build_else_arm_node(
        &mut self,
        cond_expr: &Option<Box<node::Expr>>,
        cond_arms: Vec<node::MatchArm>,
    ) -> Result<node::Expr, err::ErrKind> {
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
        crate::err_at!(self.tkn_checker())
            .close_scope_to_rbrace(Some(lex::Tkn::KeyWordCond))?;
        //}
        self.next_tkn(&[])?;
        let node = node::Expr::Match {
            pattern: cond_expr.clone(),
            arms: cond_arms,
            arm_else: Some(body),
        };
        return Ok(node);
    }

    /// 与えられた式が真偽値を返す(比較)式かどうか
    ///
    /// `match` に渡された対象の式がこれに該当する場合、
    /// 「真偽値を与える」形式(単純なif/else)として扱う
    fn is_bool_expr(expr: &node::Expr) -> bool {
        matches!(
            expr,
            node::Expr::LessThen(_)
                | node::Expr::GreaterThen(_)
                | node::Expr::Equal(_)
                | node::Expr::NotEq(_)
        )
    }

    /// `match` に真偽値(比較式)が渡された場合の解析
    /// ```
    /// cond a == 10 {
    ///     // a が 10 のとき
    /// } else {
    ///     // それ以外
    /// }
    /// ```
    /// これは `if` / `else` と同じ意味を持つ
    fn expr_match_bool(&mut self, cond: node::Expr) -> Result<node::Expr, err::ErrKind> {
        // match expr {
        // {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return crate::err_at!(self.cond_expr_scope_start_lbrace());
        }
        self.next_tkn(&[])?;
        // trueのときの処理
        let body = self.gen_block_node()?;
        // }
        // } else {
        crate::err_at!(self.tkn_checker()).close_scope_to_rbrace(None)?;
        // else
        if self.next_tkn(&["else"])? != lex::Tkn::KeyWordElse {
            return crate::err_at!(self.cond_else_not_found());
        }
        // {
        if !matches!(self.next_tkn(&["{"])?, lex::Tkn::LBrace) {
            return crate::err_at!(self.cond_else_lbrace());
        }
        self.next_tkn(&[])?;
        // それ以外のときの処理
        let else_body = self.gen_block_node()?;
        // }
        crate::err_at!(self.tkn_checker()).close_scope_to_rbrace(None)?;
        // 式の終了
        self.next_tkn(&[])?;

        Ok(node::Expr::Match {
            pattern: None,
            arms: vec![node::MatchArm {
                pattern: Box::new(cond),
                body,
            }],
            arm_else: Some(else_body),
        })
    }
}