//! 反復処理: `while` / `do-while` / `for` と、ループを制御する `break` / `continue`
//!
//! どの関数も、先頭のキーワードが未読の状態で呼ぶ。`line`は文の先頭の行番号。

use super::*;

impl Parser {
    /// `while (cond) stmt`
    pub(super) fn parse_while_stmt(&mut self, line: usize) -> PResult<CStmt> {
        self.bump();
        self.expect_punct("(")?;
        let cond = self.parse_expr()?;
        self.expect_punct(")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(CStmt {
            line,
            kind: CStmtKind::While(cond, body),
        })
    }

    /// `do stmt while (cond);`
    pub(super) fn parse_do_while_stmt(&mut self, line: usize) -> PResult<CStmt> {
        self.bump();
        let body = Box::new(self.parse_stmt()?);
        if !self.eat_kw("while") {
            return Err(self.syntax("`do`の後ろに`while`が必要です"));
        }
        self.expect_punct("(")?;
        let cond = self.parse_expr()?;
        self.expect_punct(")")?;
        self.expect_punct(";")?;
        Ok(CStmt {
            line,
            kind: CStmtKind::DoWhile(body, cond),
        })
    }

    /// `for (init; cond; step) stmt`(`init`は宣言でもよい)
    pub(super) fn parse_for_stmt(&mut self, line: usize) -> PResult<CStmt> {
        self.bump();
        self.expect_punct("(")?;
        let init = if self.eat_punct(";") {
            None
        } else if self.is_type_start() {
            Some(Box::new(self.parse_decl_stmt()?))
        } else {
            let l = self.line();
            let e = self.parse_expr()?;
            self.expect_punct(";")?;
            Some(Box::new(CStmt {
                line: l,
                kind: CStmtKind::Expr(Some(e)),
            }))
        };
        let cond = if self.is_punct(";") {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect_punct(";")?;
        let step = if self.is_punct(")") {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect_punct(")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(CStmt {
            line,
            kind: CStmtKind::For {
                init,
                cond,
                step,
                body,
            },
        })
    }

    /// `break;`
    pub(super) fn parse_break_stmt(&mut self, line: usize) -> PResult<CStmt> {
        self.bump();
        self.expect_punct(";")?;
        Ok(CStmt {
            line,
            kind: CStmtKind::Break,
        })
    }

    /// `continue;`
    pub(super) fn parse_continue_stmt(&mut self, line: usize) -> PResult<CStmt> {
        self.bump();
        self.expect_punct(";")?;
        Ok(CStmt {
            line,
            kind: CStmtKind::Continue,
        })
    }
}
