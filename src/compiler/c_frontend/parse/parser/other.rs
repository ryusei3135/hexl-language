//! その他: トークン操作・文の振り分け・式・定数式の評価・リテラルの展開

use super::*;

const BIN_OPS: &[(&str, BinOp, u8)] = &[
    ("||", BinOp::LogOr, 1),
    ("&&", BinOp::LogAnd, 2),
    ("|", BinOp::BitOr, 3),
    ("^", BinOp::BitXor, 4),
    ("&", BinOp::BitAnd, 5),
    ("==", BinOp::Eq, 6),
    ("!=", BinOp::Ne, 6),
    ("<", BinOp::Lt, 7),
    (">", BinOp::Gt, 7),
    ("<=", BinOp::Le, 7),
    (">=", BinOp::Ge, 7),
    ("<<", BinOp::Shl, 8),
    (">>", BinOp::Shr, 8),
    ("+", BinOp::Add, 9),
    ("-", BinOp::Sub, 9),
    ("*", BinOp::Mul, 10),
    ("/", BinOp::Div, 10),
    ("%", BinOp::Mod, 10),
];

const ASSIGN_OPS: &[(&str, Option<BinOp>)] = &[
    ("=", None),
    ("+=", Some(BinOp::Add)),
    ("-=", Some(BinOp::Sub)),
    ("*=", Some(BinOp::Mul)),
    ("/=", Some(BinOp::Div)),
    ("%=", Some(BinOp::Mod)),
    ("<<=", Some(BinOp::Shl)),
    (">>=", Some(BinOp::Shr)),
    ("&=", Some(BinOp::BitAnd)),
    ("^=", Some(BinOp::BitXor)),
    ("|=", Some(BinOp::BitOr)),
];

impl Parser {
    // ---------------------------------------------------------------
    // トークン操作
    // ---------------------------------------------------------------

    pub(super) fn cur(&self) -> &TokenInfo {
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }
    pub(super) fn peek_at(&self, n: usize) -> &Token {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].token
    }
    pub(super) fn tok(&self) -> &Token {
        self.peek_at(0)
    }
    pub(super) fn line(&self) -> usize {
        self.cur().line
    }
    pub(super) fn bump(&mut self) {
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
    }
    pub(super) fn is_punct(&self, p: &str) -> bool {
        matches!(self.tok(), Token::Punct { name } if *name == p)
    }
    pub(super) fn is_punct_at(&self, n: usize, p: &str) -> bool {
        matches!(self.peek_at(n), Token::Punct { name } if *name == p)
    }
    pub(super) fn is_kw(&self, k: &str) -> bool {
        matches!(self.tok(), Token::KeyWord { name } if name == k)
    }
    pub(super) fn eat_punct(&mut self, p: &str) -> bool {
        if self.is_punct(p) {
            self.bump();
            true
        } else {
            false
        }
    }
    pub(super) fn eat_kw(&mut self, k: &str) -> bool {
        if self.is_kw(k) {
            self.bump();
            true
        } else {
            false
        }
    }
    pub(super) fn expect_punct(&mut self, p: &str) -> PResult<()> {
        if self.eat_punct(p) {
            Ok(())
        } else {
            Err(self.syntax(format!("`{}`が必要です(実際: {})", p, self.describe())))
        }
    }
    pub(super) fn describe(&self) -> String {
        match self.tok() {
            Token::Eof => "ファイルの終わり".to_string(),
            t => format!("{:?}", t),
        }
    }
    pub(super) fn syntax(&self, msg: impl Into<String>) -> ParseError {
        let c = self.cur();
        ParseError::syntax(c.line, c.col, msg)
    }
    pub(super) fn unsupported(&self, msg: impl Into<String>) -> ParseError {
        let c = self.cur();
        ParseError::unsupported(c.line, c.col, msg)
    }
    pub(super) fn expect_ident(&mut self) -> PResult<String> {
        match self.tok().clone() {
            Token::Ident { name } => {
                self.bump();
                Ok(name)
            }
            _ => Err(self.syntax(format!("識別子が必要です(実際: {})", self.describe()))),
        }
    }

    /// `__attribute__((..))` `__extension__` `__asm__(..)` などを読み飛ばす
    pub(super) fn skip_gnu_noise(&mut self) {
        loop {
            match self.tok() {
                Token::Ident { name } if name == "__extension__" => self.bump(),
                Token::Ident { name }
                    if name == "__attribute__" || name == "__asm__" || name == "__asm" =>
                {
                    self.bump();
                    if self.is_punct("(") {
                        self.skip_balanced_parens();
                    }
                }
                _ => break,
            }
        }
    }

    /// 現在位置の`(`から対応する`)`の次まで進む
    pub(super) fn skip_balanced_parens(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.tok() {
                Token::Punct { name: "(" } => depth += 1,
                Token::Punct { name: ")" } => {
                    depth -= 1;
                    if depth == 0 {
                        self.bump();
                        return;
                    }
                }
                Token::Eof => return,
                _ => {}
            }
            self.bump();
        }
    }

    pub(super) fn skip_static_assert(&mut self) {
        self.bump();
        if self.is_punct("(") {
            self.skip_balanced_parens();
        }
        self.eat_punct(";");
        self.warnings
            .push(format!("{}行目: _Static_assertは無視しました", self.line()));
    }

    // ---------------------------------------------------------------
    // 文
    // ---------------------------------------------------------------

    /// 文を1つ読む。種類ごとの処理は、`function.rs`(`return`)・`iteration.rs`・`conditional.rs`・`data_type.rs`(宣言)に任せる
    pub(super) fn parse_stmt(&mut self) -> PResult<CStmt> {
        let line = self.line();
        let mk = |kind| CStmt { line, kind };
        self.skip_gnu_noise();

        // ラベル
        if let Token::Ident { name } = self.tok().clone() {
            if self.is_punct_at(1, ":") && !self.is_typedef_name(&name) {
                self.bump();
                self.bump();
                return Ok(mk(CStmtKind::Label(name)));
            }
        }

        match self.tok().clone() {
            Token::Punct { name: "{" } => {
                let items = self.parse_compound_items()?;
                Ok(mk(CStmtKind::Block(items)))
            }
            Token::Punct { name: ";" } => {
                self.bump();
                Ok(mk(CStmtKind::Expr(None)))
            }
            Token::KeyWord { name } => match name.as_str() {
                "if" => self.parse_if_stmt(line),
                "switch" => self.parse_switch_stmt(line),
                "case" => self.parse_case_label(line),
                "default" => self.parse_default_label(line),
                "while" => self.parse_while_stmt(line),
                "do" => self.parse_do_while_stmt(line),
                "for" => self.parse_for_stmt(line),
                "break" => self.parse_break_stmt(line),
                "continue" => self.parse_continue_stmt(line),
                "return" => self.parse_return_stmt(line),
                "goto" => {
                    self.bump();
                    let l = self.expect_ident()?;
                    self.expect_punct(";")?;
                    Ok(mk(CStmtKind::Goto(l)))
                }
                "_Static_assert" => {
                    self.skip_static_assert();
                    Ok(mk(CStmtKind::Expr(None)))
                }
                _ if self.is_type_start() => self.parse_decl_stmt(),
                _ => self.parse_expr_stmt(),
            },
            Token::Ident { .. } if self.is_type_start() => self.parse_decl_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    pub(super) fn parse_expr_stmt(&mut self) -> PResult<CStmt> {
        let line = self.line();
        let e = self.parse_expr()?;
        self.expect_punct(";")?;
        Ok(CStmt {
            line,
            kind: CStmtKind::Expr(Some(e)),
        })
    }

    // ---------------------------------------------------------------
    // 式
    // ---------------------------------------------------------------

    pub(super) fn mk(&self, line: usize, kind: CExprKind) -> CExpr {
        CExpr { line, kind }
    }

    pub fn parse_expr(&mut self) -> PResult<CExpr> {
        let line = self.line();
        let first = self.parse_assign()?;
        if !self.is_punct(",") {
            return Ok(first);
        }
        let mut list = vec![first];
        while self.eat_punct(",") {
            list.push(self.parse_assign()?);
        }
        Ok(self.mk(line, CExprKind::Comma(list)))
    }

    pub fn parse_assign(&mut self) -> PResult<CExpr> {
        let line = self.line();
        let lhs = self.parse_cond()?;
        if let Token::Punct { name } = self.tok() {
            if let Some((_, op)) = ASSIGN_OPS.iter().find(|(s, _)| s == name) {
                let op = *op;
                self.bump();
                let rhs = self.parse_assign()?;
                return Ok(self.mk(line, CExprKind::Assign(op, Box::new(lhs), Box::new(rhs))));
            }
        }
        Ok(lhs)
    }

    pub(super) fn parse_binary(&mut self, min_prec: u8) -> PResult<CExpr> {
        let line = self.line();
        let mut lhs = self.parse_cast()?;
        loop {
            let (op, prec) = match self.tok() {
                Token::Punct { name } => match BIN_OPS.iter().find(|(s, _, _)| s == name) {
                    Some((_, op, p)) if *p >= min_prec => (*op, *p),
                    _ => break,
                },
                _ => break,
            };
            self.bump();
            let rhs = self.parse_binary(prec + 1)?;
            lhs = self.mk(line, CExprKind::Binary(op, Box::new(lhs), Box::new(rhs)));
        }
        Ok(lhs)
    }

    pub(super) fn parse_cast(&mut self) -> PResult<CExpr> {
        let line = self.line();
        if self.is_punct("(") && self.is_type_start_at(1) {
            self.bump();
            let ty = self.parse_type_name()?;
            self.expect_punct(")")?;
            if self.is_punct("{") {
                return Err(self.unsupported("複合リテラル `(T){...}`"));
            }
            let e = self.parse_cast()?;
            return Ok(self.mk(line, CExprKind::Cast(ty, Box::new(e))));
        }
        self.parse_unary()
    }

    pub(super) fn parse_unary(&mut self) -> PResult<CExpr> {
        let line = self.line();
        let op = match self.tok() {
            Token::Punct { name } => match *name {
                "++" => Some(UnOp::PreInc),
                "--" => Some(UnOp::PreDec),
                "&" => Some(UnOp::Addr),
                "*" => Some(UnOp::Deref),
                "+" => Some(UnOp::Plus),
                "-" => Some(UnOp::Neg),
                "~" => Some(UnOp::BitNot),
                "!" => Some(UnOp::Not),
                _ => None,
            },
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let e = self.parse_cast()?;
            return Ok(self.mk(line, CExprKind::Unary(op, Box::new(e))));
        }
        if self.is_kw("sizeof") {
            self.bump();
            if self.is_punct("(") && self.is_type_start_at(1) {
                self.bump();
                let ty = self.parse_type_name()?;
                self.expect_punct(")")?;
                return Ok(self.mk(line, CExprKind::SizeofType(ty)));
            }
            let e = self.parse_unary()?;
            return Ok(self.mk(line, CExprKind::SizeofExpr(Box::new(e))));
        }
        if self.is_kw("_Alignof") {
            return Err(self.unsupported("_Alignof"));
        }
        self.parse_postfix()
    }

    pub(super) fn parse_postfix(&mut self) -> PResult<CExpr> {
        let line = self.line();
        let mut e = self.parse_primary()?;
        loop {
            if self.eat_punct("[") {
                let idx = self.parse_expr()?;
                self.expect_punct("]")?;
                e = self.mk(line, CExprKind::Index(Box::new(e), Box::new(idx)));
            } else if self.eat_punct("(") {
                let mut args = Vec::new();
                if !self.is_punct(")") {
                    loop {
                        args.push(self.parse_assign()?);
                        if !self.eat_punct(",") {
                            break;
                        }
                    }
                }
                self.expect_punct(")")?;
                e = self.mk(line, CExprKind::Call(Box::new(e), args));
            } else if self.eat_punct(".") {
                let m = self.expect_ident()?;
                e = self.mk(line, CExprKind::Member(Box::new(e), m, false));
            } else if self.eat_punct("->") {
                let m = self.expect_ident()?;
                e = self.mk(line, CExprKind::Member(Box::new(e), m, true));
            } else if self.eat_punct("++") {
                e = self.mk(line, CExprKind::Unary(UnOp::PostInc, Box::new(e)));
            } else if self.eat_punct("--") {
                e = self.mk(line, CExprKind::Unary(UnOp::PostDec, Box::new(e)));
            } else {
                break;
            }
        }
        Ok(e)
    }

    pub(super) fn parse_primary(&mut self) -> PResult<CExpr> {
        let line = self.line();
        match self.tok().clone() {
            Token::Ident { name } => {
                self.bump();
                if let Some((ename, variant, _)) = self.enum_consts.get(&name) {
                    return Ok(self.mk(
                        line,
                        CExprKind::EnumConst {
                            ename: ename.clone(),
                            variant: variant.clone(),
                        },
                    ));
                }
                Ok(self.mk(line, CExprKind::Ident(name)))
            }
            Token::IntLiteral { value, .. } => {
                self.bump();
                Ok(self.mk(line, CExprKind::Int(value.to_string())))
            }
            Token::FloatLiteral { value, .. } => {
                self.bump();
                Ok(self.mk(line, CExprKind::Float(fmt_float(value))))
            }
            Token::CharLiteral { prefix, body } => {
                if !prefix.is_empty() {
                    return Err(self.unsupported("ワイド文字定数(L'' u'' U'')"));
                }
                let v = decode_char(&body).map_err(|m| self.syntax(m))?;
                self.bump();
                Ok(self.mk(line, CExprKind::Char(v)))
            }
            Token::StringLiteral { .. } => {
                let mut s = String::new();
                while let Token::StringLiteral { prefix, body } = self.tok().clone() {
                    if !prefix.is_empty() {
                        return Err(
                            self.unsupported("ワイド/UTF文字列リテラル(L\"\" u\"\" U\"\" u8\"\")")
                        );
                    }
                    s.push_str(&decode_string(&body).map_err(|m| self.syntax(m))?);
                    self.bump();
                }
                Ok(self.mk(line, CExprKind::Str(s)))
            }
            Token::Punct { name: "(" } => {
                if self.is_punct_at(1, "{") {
                    return Err(self.unsupported("GNUの文式 `({ ... })`"));
                }
                self.bump();
                let e = self.parse_expr()?;
                self.expect_punct(")")?;
                Ok(e)
            }
            Token::KeyWord { name } if name == "_Generic" => Err(self.unsupported("_Generic")),
            _ => Err(self.syntax(format!("式が必要です(実際: {})", self.describe()))),
        }
    }

    // ---------------------------------------------------------------
    // 定数式の評価(配列の大きさ・列挙子の値・指示子の添字)
    // ---------------------------------------------------------------

    pub(super) fn const_eval(&self, e: &CExpr) -> PResult<i64> {
        let fail = |m: &str| {
            ParseError::unsupported(e.line, 0, format!("整数定数式として評価できません: {}", m))
        };
        Ok(match &e.kind {
            CExprKind::Int(s) => s.parse::<u64>().map_err(|_| fail("整数が大きすぎます"))? as i64,
            CExprKind::Char(v) => *v,
            CExprKind::EnumConst { ename, variant } => self
                .enum_consts
                .values()
                .find(|(en, v, _)| en == ename && v == variant)
                .map(|(_, _, n)| *n)
                .ok_or_else(|| fail("列挙定数"))?,
            CExprKind::Cast(_, inner) => self.const_eval(inner)?,
            CExprKind::Unary(op, a) => {
                let a = self.const_eval(a)?;
                match op {
                    UnOp::Plus => a,
                    UnOp::Neg => a.wrapping_neg(),
                    UnOp::BitNot => !a,
                    UnOp::Not => (a == 0) as i64,
                    _ => return Err(fail("単項演算子")),
                }
            }
            CExprKind::Binary(op, a, b) => {
                let (a, b) = (self.const_eval(a)?, self.const_eval(b)?);
                match op {
                    BinOp::Mul => a.wrapping_mul(b),
                    BinOp::Div => {
                        if b == 0 {
                            return Err(fail("0で割っています"));
                        }
                        a.wrapping_div(b)
                    }
                    BinOp::Mod => {
                        if b == 0 {
                            return Err(fail("0で割っています"));
                        }
                        a.wrapping_rem(b)
                    }
                    BinOp::Add => a.wrapping_add(b),
                    BinOp::Sub => a.wrapping_sub(b),
                    BinOp::Shl => a.wrapping_shl(b as u32),
                    BinOp::Shr => a.wrapping_shr(b as u32),
                    BinOp::Lt => (a < b) as i64,
                    BinOp::Gt => (a > b) as i64,
                    BinOp::Le => (a <= b) as i64,
                    BinOp::Ge => (a >= b) as i64,
                    BinOp::Eq => (a == b) as i64,
                    BinOp::Ne => (a != b) as i64,
                    BinOp::BitAnd => a & b,
                    BinOp::BitXor => a ^ b,
                    BinOp::BitOr => a | b,
                    BinOp::LogAnd => (a != 0 && b != 0) as i64,
                    BinOp::LogOr => (a != 0 || b != 0) as i64,
                }
            }
            CExprKind::Cond(c, a, b) => {
                if self.const_eval(c)? != 0 {
                    self.const_eval(a)?
                } else {
                    self.const_eval(b)?
                }
            }
            CExprKind::SizeofType(_) | CExprKind::SizeofExpr(_) => return Err(fail("sizeof")),
            _ => return Err(fail("定数でない式")),
        })
    }
}

fn fmt_float(v: f64) -> String {
    let s = format!("{:?}", v);
    if s.contains('.') || s.contains("inf") || s.contains("NaN") {
        return s;
    }
    if let Some(i) = s.find('e') {
        let (m, e) = s.split_at(i);
        return format!("{}.0{}", m, e);
    }
    format!("{}.0", s)
}

/// エスケープを展開して、1文字ぶんずつバイト列(コードポイント)にする
fn decode_escapes(body: &str) -> Result<Vec<u32>, String> {
    let chars: Vec<char> = body.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        if c != '\\' {
            out.push(c as u32);
            continue;
        }
        let Some(&e) = chars.get(i) else {
            return Err("文字列の末尾に`\\`があります".to_string());
        };
        i += 1;
        let v = match e {
            'n' => 10,
            't' => 9,
            'r' => 13,
            'a' => 7,
            'b' => 8,
            'f' => 12,
            'v' => 11,
            'e' => 27,
            '\\' => 92,
            '\'' => 39,
            '"' => 34,
            '?' => 63,
            'x' => {
                let mut v: u32 = 0;
                let mut n = 0;
                while let Some(d) = chars.get(i).and_then(|c| c.to_digit(16)) {
                    v = v.wrapping_mul(16).wrapping_add(d);
                    i += 1;
                    n += 1;
                }
                if n == 0 {
                    return Err("`\\x`の後ろに16進数が必要です".to_string());
                }
                v & 0xff
            }
            '0'..='7' => {
                let mut v = e.to_digit(8).unwrap();
                let mut n = 1;
                while n < 3 {
                    match chars.get(i).and_then(|c| c.to_digit(8)) {
                        Some(d) => {
                            v = v * 8 + d;
                            i += 1;
                            n += 1;
                        }
                        None => break,
                    }
                }
                v & 0xff
            }
            'u' | 'U' => {
                let len = if e == 'u' { 4 } else { 8 };
                let mut v: u32 = 0;
                for _ in 0..len {
                    let d = chars
                        .get(i)
                        .and_then(|c| c.to_digit(16))
                        .ok_or_else(|| "ユニバーサル文字名が不正です".to_string())?;
                    v = v * 16 + d;
                    i += 1;
                }
                v
            }
            other => return Err(format!("未知のエスケープシーケンス `\\{}`", other)),
        };
        out.push(v);
    }
    Ok(out)
}

fn decode_char(body: &str) -> Result<i64, String> {
    let cs = decode_escapes(body)?;
    if cs.is_empty() {
        return Err("空の文字定数です".to_string());
    }
    // 複数文字定数はgcc同様に先頭が上位バイト
    let mut v: i64 = 0;
    for c in &cs {
        v = (v << 8) | (*c as i64 & if cs.len() > 1 { 0xff } else { -1 });
    }
    Ok(v)
}

fn decode_string(body: &str) -> Result<String, String> {
    let cs = decode_escapes(body)?;
    Ok(cs.into_iter().filter_map(char::from_u32).collect())
}
