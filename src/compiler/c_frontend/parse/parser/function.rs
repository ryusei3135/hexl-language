//! 関数: 関数定義(トップレベル)・仮引数リスト・関数本体・`return`

use super::*;

impl Parser {
    pub(super) fn parse_external(&mut self) -> PResult<()> {
        self.skip_gnu_noise();
        if self.eat_punct(";") {
            return Ok(());
        }
        if self.is_kw("_Static_assert") {
            self.skip_static_assert();
            return Ok(());
        }
        let line = self.line();
        let specs = self.parse_specs(true)?;
        self.skip_gnu_noise();
        if self.eat_punct(";") {
            return Ok(());
        }
        // 最初の宣言子
        let (name, ty) = self.declarator(specs.base.clone(), false)?;
        let name = name.expect("abstract=falseなので名前はある");
        self.skip_gnu_noise();

        if !specs.is_typedef && self.is_punct("{") {
            if let CType::Func {
                ret,
                params,
                variadic,
            } = ty
            {
                self.items.push(CItem::Func(CFunc {
                    line,
                    name,
                    ret: *ret,
                    params,
                    variadic,
                    is_static: specs.storage == Storage::Static,
                    body: Vec::new(),
                }));
                let idx = self.items.len() - 1;
                // 本体の中で定義された構造体は、関数より前に入れたいので、関数は最後に並べ直す
                let func = self.items.pop().unwrap();
                let _ = idx;
                let body = self.parse_compound_items()?;
                if let CItem::Func(mut f) = func {
                    f.body = body;
                    self.items.push(CItem::Func(f));
                }
                return Ok(());
            }
            return Err(self.syntax("関数以外の宣言に`{`が続いています"));
        }
        if !specs.is_typedef && !matches!(ty, CType::Func { .. }) && self.is_type_start() {
            return Err(self.unsupported("K&R形式(旧式)の関数定義"));
        }

        let decls = self.finish_declarators(&specs, line, name, ty, false)?;
        for d in decls {
            self.push_global(d);
        }
        Ok(())
    }

    pub(super) fn push_global(&mut self, d: CDecl) {
        if matches!(d.ty, CType::Func { .. }) {
            self.items.push(CItem::Proto { name: d.name });
        } else if d.storage == Storage::Extern {
            self.items.push(CItem::ExternVar { name: d.name });
        } else {
            self.items.push(CItem::Global(d));
        }
    }

    /// `(`の次から`)`まで
    pub(super) fn parse_params(&mut self) -> PResult<(Vec<CParam>, bool)> {
        let mut params = Vec::new();
        let mut variadic = false;
        if self.eat_punct(")") {
            return Ok((params, false));
        }
        // `(void)`
        if self.is_kw("void") && self.is_punct_at(1, ")") {
            self.bump();
            self.bump();
            return Ok((params, false));
        }
        loop {
            if self.eat_punct("...") {
                variadic = true;
                break;
            }
            if !self.is_type_start() {
                if matches!(self.tok(), Token::Ident { .. }) {
                    // K&R の識別子リスト
                    return Err(self.unsupported("K&R形式(旧式)の仮引数リスト"));
                }
                return Err(self.syntax(format!("仮引数の型が必要です(実際: {})", self.describe())));
            }
            let specs = self.parse_specs(true)?;
            let (name, ty) = self.declarator(specs.base, true)?;
            // 配列・関数の仮引数はポインタに読み替える
            let ty = match ty {
                CType::Array { inner, .. } => CType::Ptr {
                    inner,
                    konst: false,
                },
                f @ CType::Func { .. } => CType::Ptr {
                    inner: Box::new(f),
                    konst: false,
                },
                t => t,
            };
            params.push(CParam { name, ty });
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok((params, variadic))
    }

    /// `{`の次から`}`まで読んで、中の文を返す(`{`は呼び出し側が読んでいる前提ではなく、ここで読む)
    pub(super) fn parse_compound_items(&mut self) -> PResult<Vec<CStmt>> {
        self.expect_punct("{")?;
        let mut items = Vec::new();
        while !self.is_punct("}") {
            if matches!(self.tok(), Token::Eof) {
                return Err(self.syntax("`}`が見つかりません"));
            }
            items.push(self.parse_stmt()?);
        }
        self.expect_punct("}")?;
        Ok(items)
    }

    /// `return [式];`(`return`は読み込み前)
    pub(super) fn parse_return_stmt(&mut self, line: usize) -> PResult<CStmt> {
        self.bump();
        let e = if self.is_punct(";") {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect_punct(";")?;
        Ok(CStmt {
            line,
            kind: CStmtKind::Return(e),
        })
    }
}
