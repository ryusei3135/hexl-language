//! 条件分岐: `if` / `switch`(`case` / `default`ラベル)/ 三項演算子
//!
//! 文を読む関数は、先頭のキーワードが未読の状態で呼ぶ。`line`は文の先頭の行番号。

use super::*;

impl Parser {
    /// `if (cond) stmt [else stmt]`
    pub(super) fn parse_if_stmt(&mut self, line: usize) -> PResult<CStmt> {
        self.bump();
        self.expect_punct("(")?;
        let cond = self.parse_expr()?;
        self.expect_punct(")")?;
        let then = Box::new(self.parse_stmt()?);
        let els = if self.eat_kw("else") {
            Some(Box::new(self.parse_stmt()?))
        } else {
            None
        };
        Ok(CStmt {
            line,
            kind: CStmtKind::If(cond, then, els),
        })
    }

    /// `switch (cond) stmt`
    pub(super) fn parse_switch_stmt(&mut self, line: usize) -> PResult<CStmt> {
        self.bump();
        self.expect_punct("(")?;
        let cond = self.parse_expr()?;
        self.expect_punct(")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(CStmt {
            line,
            kind: CStmtKind::Switch(cond, body),
        })
    }

    /// `case 定数式:`
    pub(super) fn parse_case_label(&mut self, line: usize) -> PResult<CStmt> {
        self.bump();
        let e = self.parse_cond()?;
        if self.is_punct("...") {
            return Err(self.unsupported("case範囲(GNU拡張 `case a ... b:`)"));
        }
        self.expect_punct(":")?;
        Ok(CStmt {
            line,
            kind: CStmtKind::Case(e),
        })
    }

    /// `default:`
    pub(super) fn parse_default_label(&mut self, line: usize) -> PResult<CStmt> {
        self.bump();
        self.expect_punct(":")?;
        Ok(CStmt {
            line,
            kind: CStmtKind::Default,
        })
    }

    /// `c ? a : b`(`?`が無ければ`c`だけを返す)
    pub(super) fn parse_cond(&mut self) -> PResult<CExpr> {
        let line = self.line();
        let c = self.parse_binary(1)?;
        if self.eat_punct("?") {
            let a = self.parse_expr()?;
            self.expect_punct(":")?;
            let b = self.parse_cond()?;
            return Ok(self.mk(line, CExprKind::Cond(Box::new(c), Box::new(a), Box::new(b))));
        }
        Ok(c)
    }
}
