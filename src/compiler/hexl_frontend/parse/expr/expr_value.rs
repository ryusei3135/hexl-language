//! Parserのexpr_value関数でトークンを作成した際に
//! その次のトークンを条件次第でするめる関数を提供する
//!
use super::*;

// `call_func_expr`は、自身で`)`を読み飛ばして呼び出し式の
// 次のトークンまで進めた状態で返ってくる(構造体初期化/配列
// リテラルが`}`を消費せず呼び出し元に委ねるのとは逆の方式)。
// そのため、値が関数呼び出し(または関数呼び出しを直接
// 包んだ`Scope`/`Member`)の場合、現在のトークンはすでに
// 呼び出し式の「次」を正しく指しており、ここでさらに
// 読み進めてはいけない
fn already_positioned_after_call(v: &Expr) -> bool {
    match &v.node {
        ExprNode::CallFunc(..) | ExprNode::Scope { .. } => true,
        ExprNode::Member { target, .. } | ExprNode::PtrMember { target, .. } => {
            matches!(target.node, ExprNode::CallFunc(..))
        }
        _ => false,
    }
}

// `self.name`のようなメンバーアクセスは、メンバー名の次のトークンまで
// 進んだ状態で返ってくる。そのため次が`)`の場合、この`)`は
// 関数呼び出しの引数の終了(`f(self.name)`)であり、すでに現在の
// トークンとして指しているので、ここで読み飛ばしてはいけない
// (読み飛ばすと`call_func_expr`が`)`を見つけられず、次の`}`などで
// `, or `)`が必要です`のエラーになる)。
// `( expr )`で囲まれた式の`)`だけは、ここで読み飛ばす
fn member_already_at_rparen(v: &Expr) -> bool {
    matches!(v.node, ExprNode::Member { .. } | ExprNode::PtrMember { .. })
}

impl Parser {
    /// 現在のトークンが数字や`)`などはスキップする
    pub fn skiped_tkn(&mut self, from_paren: bool, v: &Expr) -> Result<(), err::ErrKind> {
        if already_positioned_after_call(&v) == false {
            match self.current_tkn() {
                lex::Tkn::RParen if !from_paren && member_already_at_rparen(&v) => {}
                lex::Tkn::Name(_)
                | lex::Tkn::Number(_)
                | lex::Tkn::Float(_)
                | lex::Tkn::Str(_)
                | lex::Tkn::RParen
                | lex::Tkn::RBracket => {
                    self.next_tkn(&[])?;
                }
                // `}` は、構造体初期化(`Name { .. }`)、unsafeな共用体の初期化
                // (`Type::mem { mem: .. }`)や配列リテラル
                // (`{ .. }`)を閉じる場合にのみ、ここで読み飛ばす。
                // `self.b`のようなメンバーアクセスの直後に続く`}`は
                // 呼び出し元のブロック(関数/メゾットの本体など)を
                // 閉じるトークンなので、消費せずそのまま残す
                lex::Tkn::RBrace
                    if matches!(
                        v.node,
                        ExprNode::InitStruct { .. }
                            | ExprNode::InitVariant { .. }
                            | ExprNode::Array(..)
                    ) =>
                {
                    self.next_tkn(&[])?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(in super) fn expr_value_node(&mut self, init_struct: bool) -> Result<Expr, err::ErrKind> {
        // ## 値のトークンが出たら
        // - 呼び出し元で、次のトークンに進めるのでNumberやRParenがきたら終了
        if let lex::Tkn::Name(name) = self.current_tkn().clone() {
            // 名前の位置
            let name_span = self.build_err_span();
            match self.next_tkn_ref(&[])? {
                // おそらくこれは、条件しきなので変数の名前として返す
                lex::Tkn::LBrace => {
                    self.next_tkn(&[])?;
                    return self.gen_name_node::<false>(name, init_struct);
                }
                lex::Tkn::RBrace => {
                    self.next_tkn(&[])?;
                    return self.gen_name_node::<false>(name, init_struct);
                }
                lex::Tkn::RParen => {
                    self.next_tkn(&[])?;
                    return Ok(Expr::new(ExprNode::Var(name), name_span));
                }
                lex::Tkn::Inc | lex::Tkn::Dec => {
                    let postfix = self.next_tkn(&[])?;
                    self.advance_tkn_checked()?;
                    let var = self.gen_name_node::<false>(name, init_struct)?;
                    return Ok(if postfix == lex::Tkn::Inc {
                        Expr::new(ExprNode::Inc(Box::new(var)), Rc::clone(&name_span))
                    } else {
                        Expr::new(ExprNode::Dec(Box::new(var)), Rc::clone(&name_span))
                    });
                }
                // データ付きバリアントなど
                lex::Tkn::ModPathTkn => {
                    self.advance_tkn_checked()?;
                    return self.scope_member_node(name, init_struct);
                }
                // `name->member` / `name->method(..)`は`[name].member`と同じ。
                // `name.member`(`build_scope_node`)と同じく、メンバー名(または
                // メゾット呼び出しの次)を指した状態で返す
                lex::Tkn::AccessPtr => {
                    self.next_tkn(&["->"])?;
                    let member = self.ptr_member_node(&name, None, None)?;
                    // 後置の`++`/`--`: `x = self->len++`
                    // (メンバーのフィールドのときだけ。メゾット呼び出しの次は見ない)
                    let is_field = matches!(
                        &member.node,
                        ExprNode::PtrMember { target, .. } if matches!(target.node, ExprNode::Var(_))
                    );
                    if is_field
                        && matches!(self.next_tkn_ref(&[]), Ok(lex::Tkn::Inc | lex::Tkn::Dec))
                    {
                        let postfix = self.next_tkn(&[])?;
                        self.advance_tkn_checked()?;
                        let span = Rc::clone(&member.span);
                        return Ok(Expr::new(
                            if postfix == lex::Tkn::Inc {
                                ExprNode::Inc(Box::new(member))
                            } else {
                                ExprNode::Dec(Box::new(member))
                            },
                            span,
                        ));
                    }
                    return Ok(member);
                }
                // 関数を呼ぶノード
                lex::Tkn::LParen => {
                    self.next_tkn(&[])?;
                    // スコープを作成
                    if self.current_tkn() == &lex::Tkn::Dot {
                        return self.build_scope_node(&name);
                    }
                    // 前回のトークンが名前かつ(なので、関数を呼び出すノードを作成する
                    return self.call_func_expr(&name, init_struct);
                }
                // 名前の次が`{`/`}`/`(`以外の場合、この名前自身は値ではなく、
                // 式の直前にあるだけのトークン。
                // 例: 配列アクセス`[arr a.a else 0]`の`arr`(`expr/arr_access.rs`)
                // このまま下の処理に進み、名前の次のトークンから値を読み取る
                _ => {}
            }
        }

        // 配列の中の処理は`src/parse/expr_value.rs`にある
        let value_tkn =
            self.next_tkn(&["[", "*", "number", "float", "string", "name", "(", "{"])?;
        // 値の先頭のトークン(`next_tkn`で進めた後の現在のトークン)の位置
        let span = self.build_err_span();
        // `( expr )`で囲まれた式の場合にtrue。この`)`は末尾で読み飛ばす必要がある
        let mut from_paren = false;
        let v = match value_tkn {
            // 変数のアドレスを取得するノード
            lex::Tkn::LBracket => self.get_var_addr_node()?,
            // 型のバイトサイズを取得するノード
            lex::Tkn::KeyWordSizeof => {
                self.advance_tkn().unwrap();
                let ty = self.ty_node_after_delim()?;
                self.advance_tkn().unwrap();
                return Ok(Expr::new(ExprNode::Sizeof(ty), span));
            }
            // ポインタにアクセス
            lex::Tkn::Mul => Expr::new(
                ExprNode::ConnectAddr(Box::new(self.expr_value(init_struct)?)),
                Rc::clone(&span),
            ),
            lex::Tkn::Number(value) => Expr::new(ExprNode::Number(value), span),
            lex::Tkn::Float(value) => Expr::new(ExprNode::Float(value), span),
            lex::Tkn::KeyWordSelf => {
                let Some(self_name) = self.struct_self_name.clone() else {
                    return crate::err_at!(self.self_outside_of_type());
                };
                // `self.name` / `self->name`は`gen_name_node`が処理する
                // `expr/value_api.rs`
                self.gen_name_node::<true>(self_name, init_struct)?
            }
            lex::Tkn::Str(value) => Expr::new(ExprNode::Str(value), span),
            lex::Tkn::Name(name) => {
                let var = self.gen_name_node::<false>(name, init_struct)?;
                // 変数の契約の情報(`data"len"`)。
                // - 単純な変数: `gen_name_node`は名前を現在のトークンのまま返すので、
                //   契約の文字列は次のトークン。ここで`"len"`まで進めておく
                //   (`skiped_tkn`が、その次の`,`や`)`まで進める)
                // - 呼び出しなど: すでに値の次のトークンまで進んでいるので、現在のトークン
                let contract = match &var.node {
                    ExprNode::Var(_) => match self.next_tkn_ref(&[]) {
                        Ok(lex::Tkn::Str(contract)) => {
                            self.advance_tkn_checked()?;
                            Some(contract)
                        }
                        _ => None,
                    },
                    _ => match self.current_tkn() {
                        lex::Tkn::Str(contract) => Some(contract.clone()),
                        _ => None,
                    },
                };
                if let Some(contract) = contract {
                    // 変数の契約の情報や設定をするもの
                    match contract.as_str() {
                        "len" => Expr::new(ExprNode::GetContractMemLen(Box::new(var)), span),
                        opt => self.this_contract_opt_is_not_found(opt.to_string())?,
                    }
                } else {
                    match self.current_tkn() {
                        lex::Tkn::Inc => {
                            self.advance_tkn();
                            return Ok(Expr::new(ExprNode::Inc(Box::new(var)), span));
                        }
                        lex::Tkn::Dec => {
                            self.advance_tkn();
                            return Ok(Expr::new(ExprNode::Dec(Box::new(var)), span));
                        }
                        _ => var,
                    }
                }
            }
            lex::Tkn::LParen => {
                // キャスト: `(T)var` -> `Cast((T, Box(var)))`
                // 現在のトークンは`(`。`(` `Name` `)` `Name`の並びのときだけキャストとみなす
                // (`(a) - b`や`(a) * b`のような、括弧で囲んだ式とは区別される)
                if matches!(self.peek_tkn()?, lex::Tkn::Name(_))
                    && self.peek2_tkn() == Some(lex::Tkn::RParen)
                    && matches!(self.peek3_tkn(), Some(lex::Tkn::Name(_)))
                {
                    // `T`、`)`と進めて、現在のトークンを`)`にする。
                    // `expr_value`は現在のトークンの次から値を読む。
                    // 値を読み終えた後の位置の調整は`expr_value`が行うので、
                    // ここでは追加で進めずにそのまま返す
                    let lex::Tkn::Name(ty_name) = self.next_tkn(&["name"])? else {
                        return crate::err_at!(self.internal_state_err("cast type name"));
                    };
                    self.next_tkn(&[")"])?;
                    let var = self.expr_value(init_struct)?;
                    return Ok(Expr::new(
                        ExprNode::Cast((TyNode::Ty(ty_name), Box::new(var))),
                        span,
                    ));
                }

                let result = self.expr_or(init_struct)?;

                if self.current_tkn() == &lex::Tkn::LParen {
                    dbg!(self.current_tkn());
                    return crate::err_at!(self.unexpect_tkn_in_expr());
                } else {
                    from_paren = true;
                    result
                }
            }
            // 配列リテラル: `{100, 100, 100, 100}`
            lex::Tkn::LBrace => self.make_array_node()?,
            t => {
                return crate::err_at!(self.expected_tkn(
                    "value (`[`, `*`, number, float, string, name, `(` or `{`)",
                    t
                ));
            }
        };

        self.skiped_tkn(from_paren, &v)?;
        Ok(v)
    }
}