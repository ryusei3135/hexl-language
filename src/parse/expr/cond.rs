use super::*;


impl Parser {
    /// 最初にキーワードのmatchが来る必要がある
    pub(in crate::parse) fn expr_match(
        &mut self
    ) -> Result<node::Expr, err::ErrKind> {
        // match の対象式
        let target = if matches!(self.current_tkn(), lex::Tkn::LBrace) {
            None
        } else {
            Some(Box::new(self.expr_cmp()?))
        };

        // 真偽値(比較式)が与えられた場合は、単純なif/elseとして扱う
        // `match a == 10 { .. } | => { .. }`
        if let Some(ref target_expr) = target {
            if Self::is_bool_expr(target_expr) {
                let cond = *target.unwrap();
                return self.expr_match_bool(cond);
            }
        }

        // {
        if !matches!(self.current_tkn(), lex::Tkn::LBrace) {
            return Err(err::ErrKind::UnexpectedToken);
        }

        let mut arms = Vec::new();

        loop {
            // }
            if matches!(self.current_tkn(), lex::Tkn::RBrace) {
                break;
            }
            // パターン else 
            if self.current_tkn() == &lex::Tkn::Or {
                // =>
                if !matches!(self.next_tkn(vec!["=>"])?, lex::Tkn::Arrow) {
                    return Err(err::ErrKind::UnexpectedToken);
                }
                // {
                if !matches!(self.next_tkn(vec!["{"])?, lex::Tkn::LBrace) {
                    return Err(err::ErrKind::UnexpectedToken);
                }
                self.next_tkn(vec![])?;
                // ここでアーム本体を解析
                let body = self.gen_block_node()?;
                // }
                if !matches!(self.current_tkn(), lex::Tkn::RBrace) {
                    return Err(err::ErrKind::UnexpectedToken);
                }
                // }
                if !matches!(self.next_tkn(vec!["}"])?, lex::Tkn::RBrace) {
                    panic!("e");
                }
                //}
                self.next_tkn(vec![])?;
                let node = node::Expr::Match {
                    pattern: target,
                    arms,
                    arm_else: Some(body),
                };
                return Ok(node);
            }
            // if 
            let pattern = self.expr_cmp()?;

            // =>
            if !matches!(self.current_tkn(), lex::Tkn::Arrow) {
                return Err(err::ErrKind::UnexpectedToken);
            }
            // {
            if !matches!(self.next_tkn(vec!["{"])?, lex::Tkn::LBrace) {
                return Err(err::ErrKind::UnexpectedToken);
            }
            self.next_tkn(vec![])?;
            // ここでアーム本体を解析
            let body = self.gen_block_node()?;
            // }
            if !matches!(self.current_tkn(), lex::Tkn::RBrace) {
                return Err(err::ErrKind::UnexpectedToken);
            }
            self.next_tkn(vec![])?;
            arms.push(node::MatchArm {
                pattern: Box::new(pattern),
                body,
            });
        }

        // 式の終了
        self.next_tkn(vec!["}"])?;//}

        let node = node::Expr::Match {
            pattern: target,
            arms,
            arm_else: None,
        };
        Ok(node)
    }

    /// 与えられた式が真偽値を返す(比較)式かどうか
    ///
    /// `match` に渡された対象の式がこれに該当する場合、
    /// 「真偽値を与える」形式(単純なif/else)として扱う
    fn is_bool_expr(expr: &node::Expr) -> bool {
        matches!(
            expr,
            node::Expr::LessThen(_) | node::Expr::GreaterThen(_) | node::Expr::Equal(_)
        )
    }

    /// `match` に真偽値(比較式)が渡された場合の解析
    /// ```
    /// match a == 10 {
    ///     // a が 10 のとき
    /// } | => {
    ///     // それ以外
    /// }
    /// ```
    /// これは `if` / `else` と同じ意味を持つ
    fn expr_match_bool(
        &mut self,
        cond: node::Expr
    ) -> Result<node::Expr, err::ErrKind> {
        // =>
        if !matches!(self.current_tkn(), lex::Tkn::Arrow) {
            return Err(err::ErrKind::UnexpectedToken);
        }
        // {
        if !matches!(self.next_tkn(vec!["{"])?, lex::Tkn::LBrace) {
            return Err(err::ErrKind::UnexpectedToken);
        }
        self.next_tkn(vec![])?;
        // trueのときの処理
        let body = self.gen_block_node()?;
        // }
        if !matches!(self.current_tkn(), lex::Tkn::RBrace) {
            return Err(err::ErrKind::UnexpectedToken);
        }
        // |
        if !matches!(self.next_tkn(vec!["|"])?, lex::Tkn::Or) {
            return Err(err::ErrKind::UnexpectedToken);
        }
        // =>
        if !matches!(self.next_tkn(vec!["=>"])?, lex::Tkn::Arrow) {
            return Err(err::ErrKind::UnexpectedToken);
        }
        // {
        if !matches!(self.next_tkn(vec!["{"])?, lex::Tkn::LBrace) {
            return Err(err::ErrKind::UnexpectedToken);
        }
        self.next_tkn(vec![])?;
        // それ以外のときの処理
        let else_body = self.gen_block_node()?;
        // }
        if !matches!(self.current_tkn(), lex::Tkn::RBrace) {
            return Err(err::ErrKind::UnexpectedToken);
        }
        // 式の終了
        self.next_tkn(vec![])?;

        Ok(node::Expr::Match {
            pattern: None,
            arms: vec![
                node::MatchArm {
                    pattern: Box::new(cond),
                    body,
                }
            ],
            arm_else: Some(else_body),
        })
    }
}
