//! ## 引数
//! 1. ini_struct
//!     - これがtrueの場合構造体を初期化するノードを作成できる
//!     - falseの場合は初期化ノードを作成しない

use std::rc::Rc;

use super::*;

mod arr_access;
mod cond;
pub(in crate::compiler::hexl_frontend::parse) mod range;
/// このファイルでしか使われないAPIのモジュール
mod value_api;
/// `cond`の条件式の中だけで使える`M(val) := value`
mod variant_bind;
use crate::compiler::hexl_frontend::lex;

impl Parser {
    pub(in crate::compiler::hexl_frontend::parse) fn expr_define_var(
        &mut self,
        // 呼び出す前にでた、変数や関数などの名前
        name: String,
    ) -> Result<Expr, err::ErrKind> {
        // この式(名前)の位置。内側のノードも同じ位置を使う
        let start = self.build_err_span();
        let mk = |n: ExprNode| Expr::new(n, start.clone());
        let node = if self
            .next_tkn(&[
                "(", "{", ",", "[", ":", "=", "+=", "-=", "*=", "/=", "%=", "&=", "^=", "|=",
                "<<=", ">>=", "]", "++", "--",
            ])
            .map(|_| true)?
        {
            match &self.current_tkn() {
                lex::Tkn::Inc | lex::Tkn::Dec => {
                    let postfix = self.current_tkn().clone();
                    self.advance_tkn_checked()?;
                    let var = mk(ExprNode::Var(name));
                    return Ok(if postfix == lex::Tkn::Inc {
                        mk(ExprNode::Inc(Box::new(var)))
                    } else {
                        mk(ExprNode::Dec(Box::new(var)))
                    });
                }
                lex::Tkn::Dot | lex::Tkn::ModPathTkn => {
                    let dst = self.build_scope_node(&name)?;
                    println!("{:?}", self.next_tkn_ref(&[]));
                    if self.next_tkn_ref(&["="])? == lex::Tkn::Equal {
                        return Ok(AssignVar::new(
                            &name,
                            dst,
                            self.expr_branch()?,
                            start.clone(),
                        ));
                    } else {
                        return Ok(dst);
                    }
                }
                lex::Tkn::RBrace => {
                    return Ok(mk(ExprNode::Var(name)));
                }
                lex::Tkn::LParen => {
                    // 関数の呼び出しノードを生成
                    let n = self.call_func_expr(&name, true);
                    return n;
                }
                // ジェネリクス関数の呼び出し: `func<int>(..)`
                // (比較の`a < b`と区別するため、`name`が定義済みの
                // ジェネリクス関数で、`<..>(`の形のときだけ)
                lex::Tkn::LAngleBracket if self.is_generic_call(&name, self.idx) => {
                    return self.generic_call_expr(&name, true);
                }
                // ジェネリクスな構造体/バリアントの初期化・メゾットの呼び出し:
                // `Name<int> { .. }` / `Name<int>::Mem`
                lex::Tkn::LAngleBracket if self.is_generic_type_expr(&name, self.idx) => {
                    return self.generic_type_expr::<false>(&name, true);
                }
                lex::Tkn::LBrace => {
                    return self.struct_init_node::<false>(&name);
                }
                lex::Tkn::Comma => {
                    return Ok(mk(ExprNode::Var(name)));
                }
                // ポインタ参照
                lex::Tkn::RBracket => {
                    // ポインタ参照なので、次のトークンに進めずに
                    // ノードを返す
                    if !matches!(self.next_tkn_ref(&["="])?, lex::Tkn::Equal) {
                        return Ok(mk(ExprNode::Var(name)));
                    }
                    self.next_tkn(&["="])?;
                    let var = mk(ExprNode::Var(name.to_string()));
                    return Ok(AssignVar::new(
                        &name,
                        mk(ExprNode::GetAddress(Box::new(var))),
                        self.expr_branch()?,
                        start.clone(),
                    ));
                }
                _ => {}
            }

            // `Err`は、変数の定義ではなく代入(`a = 1`/`a += 1`)のノードが
            // 作られた(または、その解析でエラーになった)場合
            let var_attr = match self.assign_expr_is_mut(&name) {
                Ok(attr) => attr,
                Err(node) => return node,
            };
            let ty_node = self.define_ty_node()?;

            if matches!(self.current_tkn(), lex::Tkn::RBracket) {
                if matches!(self.next_tkn(&["="])?, lex::Tkn::Equal) {
                    let value = mk(ExprNode::ConnectAddr(Box::new(self.expr_branch()?)));
                    return Ok(
                        DefineVar::new(&name, value, &ty_node, var_attr).wrap(start.clone())
                    );
                }
            }

            if self.current_tkn() == &lex::Tkn::Equal {
                DefineVar::new(&name, self.expr_branch()?, &ty_node, var_attr).wrap(start)
            } else {
                return crate::err_at!(self.unexpect_tkn_in_expr());
            }
        } else {
            return crate::err_at!(self.tkn_is_eof_in_expr());
        };

        Ok(node)
    }

    /// 二項演算のノードを作る。位置は、左のオペランドの先頭
    fn binary_expr(
        ctor: fn((Box<Expr>, Box<Expr>)) -> ExprNode,
        left: Expr,
        right: Expr,
    ) -> Expr {
        let span = left.span.clone();
        Expr::new(ctor(Expr::wrap(left, right)), span)
    }

    /// 式に代入する物が、構文の式 例(match)かどうかで
    pub(in crate::compiler::hexl_frontend::parse) fn expr_branch(&mut self) -> Result<Expr, err::ErrKind> {
        if matches!(self.next_tkn_ref(&["match"])?, lex::Tkn::KeyWordCond) {
            self.next_tkn(&[])?;
            self.expr_match()
        } else {
            self.expr_or(true)
        }
    }

    /// 式の最上位(最も優先順位が低い条件演算子`? :`)。呼び出し元はここから式を読む
    ///
    /// 優先順位(低 -> 高):
    /// `? :` < `||` < `&&` < `|` < `^` < `&` < `==` `!=` < ...
    ///
    /// 右結合: `a ? b : c ? d : e` は `a ? b : (c ? d : e)`
    pub(in crate::compiler::hexl_frontend::parse) fn expr_or(
        &mut self,
        init_struct: bool,
    ) -> Result<Expr, err::ErrKind> {
        let cond = self.expr_cmp_or(init_struct)?;
        if self.current_tkn() != &lex::Tkn::Question {
            return Ok(cond);
        }

        // 現在のトークンは`?`。`then`は`?`の次から読む
        let then = self.expr_or(init_struct)?;
        if self.current_tkn() != &lex::Tkn::Colon {
            return crate::err_at!(self.expected_at_current(":"));
        }
        // 現在のトークンは`:`。`else`は`:`の次から読む
        let els = self.expr_or(init_struct)?;

        // 位置は、条件式の先頭
        let span = cond.span.clone();
        Ok(Expr::new(
            ExprNode::Ternary((Box::new(cond), Box::new(then), Box::new(els))),
            span,
        ))
    }

    /// `||`
    fn expr_cmp_or(&mut self, init_struct: bool) -> Result<Expr, err::ErrKind> {
        let mut left = self.expr_cmp_and(init_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::CmpOr => {
                    Self::binary_expr(ExprNode::CmpOr, left, self.expr_cmp_and(init_struct)?)
                }
                _ => break,
            };
        }

        Ok(left)
    }

    /// `&&`
    fn expr_cmp_and(&mut self, init_struct: bool) -> Result<Expr, err::ErrKind> {
        let mut left = self.expr_bit_or(init_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::CmpAnd => {
                    Self::binary_expr(ExprNode::CmpAnd, left, self.expr_bit_or(init_struct)?)
                }
                _ => break,
            };
        }

        Ok(left)
    }

    /// `|`
    fn expr_bit_or(&mut self, init_struct: bool) -> Result<Expr, err::ErrKind> {
        let mut left = self.expr_xor(init_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::Or => {
                    Self::binary_expr(ExprNode::OrCalcul, left, self.expr_xor(init_struct)?)
                }
                _ => break,
            };
        }

        Ok(left)
    }

    fn expr_xor(&mut self, init_struct: bool) -> Result<Expr, err::ErrKind> {
        let mut left = self.expr_and(init_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::Xor => {
                    Self::binary_expr(ExprNode::XorCalcul, left, self.expr_and(init_struct)?)
                }
                _ => break,
            };
        }

        Ok(left)
    }

    fn expr_and(&mut self, init_struct: bool) -> Result<Expr, err::ErrKind> {
        let mut left = self.expr_eq(init_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::And => {
                    Self::binary_expr(ExprNode::AndCalcul, left, self.expr_eq(init_struct)?)
                }
                _ => break,
            };
        }

        Ok(left)
    }

    fn expr_eq(&mut self, init_struct: bool) -> Result<Expr, err::ErrKind> {
        let mut left = self.expr_cmp(init_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::EqEq => {
                    Self::binary_expr(ExprNode::Equal, left, self.expr_cmp(init_struct)?)
                }
                lex::Tkn::NotEq => {
                    Self::binary_expr(ExprNode::NotEq, left, self.expr_cmp(init_struct)?)
                }
                _ => break,
            };
        }

        Ok(left)
    }

    fn expr_cmp(&mut self, ini_struct: bool) -> Result<Expr, err::ErrKind> {
        let mut left = self.expr_shift(ini_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::LAngleBracket => {
                    Self::binary_expr(ExprNode::LessThen, left, self.expr_shift(ini_struct)?)
                }
                lex::Tkn::RAngleBracket => Self::binary_expr(
                    ExprNode::GreaterThen,
                    left,
                    self.expr_shift(ini_struct)?,
                ),
                lex::Tkn::LessThanEq => Self::binary_expr(
                    ExprNode::LessThenEq,
                    left,
                    self.expr_shift(ini_struct)?,
                ),
                lex::Tkn::GreaterThanEq => Self::binary_expr(
                    ExprNode::GreaterThenEq,
                    left,
                    self.expr_shift(ini_struct)?,
                ),
                _ => break,
            };
        }

        Ok(left)
    }

    fn expr_shift(&mut self, init_struct: bool) -> Result<Expr, err::ErrKind> {
        let mut left = self.expr_add(init_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::LShift => {
                    Self::binary_expr(ExprNode::LShift, left, self.expr_add(init_struct)?)
                }
                lex::Tkn::RShift => {
                    Self::binary_expr(ExprNode::RShift, left, self.expr_add(init_struct)?)
                }
                _ => break,
            };
        }

        Ok(left)
    }

    pub(in crate::compiler::hexl_frontend::parse) fn expr_add(
        &mut self,
        ini_struct: bool,
    ) -> Result<Expr, err::ErrKind> {
        let mut left = self.expr_mul(ini_struct)?;

        // expr_mulですでにトークンを進めているので現在のトークンを参照
        loop {
            left = match self.current_tkn() {
                lex::Tkn::Add => {
                    Self::binary_expr(ExprNode::Add, left, self.expr_mul(ini_struct)?)
                }
                lex::Tkn::Sub => {
                    Self::binary_expr(ExprNode::Sub, left, self.expr_mul(ini_struct)?)
                }
                _ => break,
            };
        }
        Ok(left)
    }

    fn expr_mul(&mut self, ini_struct: bool) -> Result<Expr, err::ErrKind> {
        let mut left = self.expr_unary(ini_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::Mul => {
                    Self::binary_expr(ExprNode::Mul, left, self.expr_unary(ini_struct)?)
                }
                lex::Tkn::Div => {
                    Self::binary_expr(ExprNode::Div, left, self.expr_unary(ini_struct)?)
                }
                lex::Tkn::Surplus => {
                    Self::binary_expr(ExprNode::Surplus, left, self.expr_unary(ini_struct)?)
                }
                _ => break,
            };
        }
        Ok(left)
    }

    /// 前置の単項演算子: `++` `--` `+` `-` `!` `~`
    ///
    /// `expr_value`と同じ規約で、呼び出し時の現在のトークンは「値の直前の
    /// トークン」(`=`や二項演算子など)。次のトークンが単項演算子なら
    /// 1つ進めて、続くオペランドを再帰で読む(`- -a`や`!~a`も可)。
    /// 単項演算子でなければ、そのまま`expr_value`に処理を委ねる。
    ///
    /// - `++a` -> `LeftInc(a)` / `--a` -> `LeftDec(a)`
    /// - `+a`  -> `Sign((true, a))` / `-a` -> `Sign((false, a))`
    /// - `!a`  -> `CmpNot(a)` (論理反転) / `~a` -> `Inversion(a)` (ビット反転)
    ///
    /// 優先順位は乗除(`*` `/` `%`)より高い(`-a * b`は`(-a) * b`)
    fn expr_unary(&mut self, ini_struct: bool) -> Result<Expr, err::ErrKind> {
        let op = self.next_tkn_ref(&[])?;
        if !matches!(
            op,
            lex::Tkn::Inc
                | lex::Tkn::Dec
                | lex::Tkn::Add
                | lex::Tkn::Sub
                | lex::Tkn::Not
                | lex::Tkn::Inversion
        ) {
            return self.expr_value(ini_struct);
        }

        // 演算子を現在のトークンにする。オペランドは次のトークンから始まる
        self.next_tkn(&[])?;
        // 位置は、演算子
        let span = self.build_err_span();
        let operand = Box::new(self.expr_unary(ini_struct)?);
        let node = match op {
            lex::Tkn::Inc => ExprNode::LeftInc(operand),
            lex::Tkn::Dec => ExprNode::LeftDec(operand),
            lex::Tkn::Add => ExprNode::Sign((true, operand)),
            lex::Tkn::Sub => ExprNode::Sign((false, operand)),
            lex::Tkn::Not => ExprNode::CmpNot(operand),
            _ => ExprNode::Inversion(operand),
        };
        Ok(Expr::new(node, span))
    }

    fn expr_value(&mut self, init_struct: bool) -> Result<Expr, err::ErrKind> {
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
        let v = match value_tkn {
            // 変数のアドレスを取得するノード
            lex::Tkn::LBracket => self.get_var_addr_node()?,
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
                // `expr/value_api.rs`
                self.gen_name_node::<true>(self_name, init_struct)?
            }
            lex::Tkn::Str(value) => Expr::new(ExprNode::Str(value), span),
            lex::Tkn::Name(name) => {
                let var = self.gen_name_node::<false>(name, init_struct)?;
                match self.current_tkn() {
                    lex::Tkn::Inc => {
                        self.advance_tkn();
                        return Ok(Expr::new(ExprNode::Inc(Box::new(var)), span));
                    }
                    lex::Tkn::Dec => {
                        self.advance_tkn();
                        return Ok(Expr::new(ExprNode::Dec(Box::new(var)), span));
                    }
                    _ => {}
                }
                var
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
                ExprNode::Member { target, .. }
                | ExprNode::PtrMember { target, .. } => {
                    matches!(target.node, ExprNode::CallFunc(..))
                }
                _ => false,
            }
        }

        if already_positioned_after_call(&v) == false {
            match self.current_tkn() {
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

        Ok(v)
    }

    /// 関数を呼び出すノードを作成
    ///
    /// ## Args
    /// - name
    ///     関数の名前
    /// - ini_struct
    ///     これがtrueの場合のみ構造体を初期化するノードを作成可能
    ///
    /// ## Errors
    /// 現在のトークンが`lex::Tkn::LParen`でないなら`Err`を返す
    ///
    /// ## Safety
    /// この関数が実行される場合、現在のトークンが`lex::Tkn::LParen`
    /// である必要がある
    pub(in crate::compiler::hexl_frontend::parse) fn call_func_expr(
        &mut self,
        name: &str,
        ini_struct: bool,
    ) -> Result<Expr, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LParen {
            return crate::err_at!(self.expected_at_current("("));
        }
        // 関数呼び出しの位置(`(`)
        let span = self.build_err_span();
        // 引数
        let mut args = Vec::<Expr>::new();

        // 関数を呼び出す式に引数がない場合は実行されない
        if self.next_tkn_ref(&["not `)`"])? != lex::Tkn::RParen {
            loop {
                // 引数の式を取得
                args.push(self.expr_or(ini_struct)?);

                match self.current_tkn() {
                    lex::Tkn::Comma => {
                        continue;
                    }
                    lex::Tkn::RParen => {
                        // 関数の最後の部分に来たので、ループを終了する
                        break;
                    }
                    _ => {
                        return crate::err_at!(self.expected_at_current(", or `)`"));
                    }
                }
            }
        } else {
            // 引数がない場合、現在のトークンはまだ`(`のままなので、
            // 引数がある場合のループが`)`を指した状態で抜けるのに
            // 合わせて、ここで`)`まで進めておく
            self.next_tkn(&[])?;
        }

        // ')'をスキップ
        self.next_tkn(&[])?;
        Ok(Expr::new(
            ExprNode::CallFunc(CallInfo {
                name: name.to_owned(),
                // ジェネリクス関数の場合は、呼び出し元(`generic_call_expr`)が
                // `<>`の中身を入れる
                temp_ty: Vec::new(),
                args,
            }),
            span,
        ))
    }
}

#[cfg(test)]
mod expr_tests {
    use crate::compiler::{
        err, lex,
        node::{self, *},
        parse,
    };

    /// テスト用の位置。`Expr`の`PartialEq`は`span`を比較しないので、何でもよい
    fn sp() -> std::rc::Rc<err::Span> {
        std::rc::Rc::new(err::Span::new(&0, &0))
    }

    /// 位置を付けて`Expr`にする
    fn ex(n: ExprNode) -> Expr {
        Expr::new(n, sp())
    }

    fn gen_nodes(content: &str) -> Vec<lex::LocatedTkn> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&content.to_string()).unwrap();
        lexer.gen_tkns.clone()
    }

    #[test]
    fn check_get_address_var() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a: int* = [b] }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::GetAddress(Box::new(ex(
                    ExprNode::Var("b".to_string())
                ))))),
                ty: TyNode::Pointer {
                    is_const: false,
                    ty_name: Box::new(TyNode::Ty("int".to_string())),
                    range: None,
                },
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_call_func_node() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a(10, a) }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::CallFunc(CallInfo {
                name: "a".to_string(),
                temp_ty: Vec::new(),
                args: vec![
                    ex(ExprNode::Number("10".to_string())),
                    ex(ExprNode::Var("a".to_string())),
                ]
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_float_literal_arg() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a(1.5, 2) }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::CallFunc(CallInfo {
                name: "a".to_string(),
                temp_ty: Vec::new(),
                args: vec![
                    ex(ExprNode::Float("1.5".to_string())),
                    ex(ExprNode::Number("2".to_string())),
                ]
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_bitwise_operator_precedence() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: int = 1 | 2 ^ 3 & 4 }");
        let Group1Node::FuncDefine(ref function) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        let Group2Node::Expr(Expr {
            node: ExprNode::DefVar(var),
            ..
        }) = function.body[0].get_node()
        else {
            panic!("not variable definition");
        };

        let expected = ex(ExprNode::OrCalcul(Expr::wrap(
            ex(ExprNode::Number("1".to_string())),
            ex(ExprNode::XorCalcul(Expr::wrap(
                ex(ExprNode::Number("2".to_string())),
                ex(ExprNode::AndCalcul(Expr::wrap(
                    ex(ExprNode::Number("3".to_string())),
                    ex(ExprNode::Number("4".to_string())),
                ))),
            ))),
        )));
        assert_eq!(var.value.as_ref(), &expected);
    }

    #[test]
    fn check_bitwise_operator_in_return() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { ret 1 & 2 }");
        let Group1Node::FuncDefine(ref function) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };

        let expected = StmtNode::Return(ex(ExprNode::AndCalcul(Expr::wrap(
            ex(ExprNode::Number("1".to_string())),
            ex(ExprNode::Number("2".to_string())),
        ))))
        .wrap();
        assert_eq!(function.body[0].get_node(), &expected);
    }

    #[test]
    fn check_stack_var_single() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: [int] = 100 }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::Number("100".to_string()))),
                ty: TyNode::Stack {
                    name: "int".to_string(),
                    len: 1
                },
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_stack_var_array() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: [int 4] = {100, 100, 100, 100} }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::Array(vec![
                    ex(ExprNode::Number("100".to_string())),
                    ex(ExprNode::Number("100".to_string())),
                    ex(ExprNode::Number("100".to_string())),
                    ex(ExprNode::Number("100".to_string())),
                ]))),
                ty: TyNode::Stack {
                    name: "int".to_string(),
                    len: 4
                },
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_static_var_single() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: static[int] = 100 }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::Number("100".to_string()))),
                ty: TyNode::Static {
                    name: "int".to_string(),
                    len: 1
                },
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_match_expr() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(
            "
            main(): int {
              cond {
                10 == 10 => {
                  hh: b1 = 100
                }
                else=> {
                  a: b1 = 10
                }
              }
            }
            ",
        );
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::Match {
                pattern: None,
                arms: vec![MatchArm {
                    pattern: Box::new(wrap_eq_expr_cmp("10", "10")),
                    body: vec![gen_var_node("hh", "100", "b1", 5),],
                }],
                arm_else: Some(vec![gen_var_node("a", "10", "b1", 8),]),
            })
            .wrap_group2()
        );
    }

    #[test]
    fn check_match_expr_bool() {
        // 1. Booleanを与えるパターン (単純なif/else)
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(
            "
            main(): int {
              cond a == 10 {
                hh: int = 100
              } else {
                a: int = 10
              }
            }
            ",
        );
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::Match {
                pattern: None,
                arms: vec![MatchArm {
                    pattern: Box::new(ex(ExprNode::Equal((
                        Box::new(ex(ExprNode::Var("a".to_string()))),
                        Box::new(ex(ExprNode::Number("10".to_string()))),
                    )))),
                    body: vec![gen_var_node("hh", "100", "int", 4),],
                }],
                arm_else: Some(vec![gen_var_node("a", "10", "int", 6),]),
            })
            .wrap_group2()
        );
    }

    #[test]
    fn check_match_expr_value() {
        // 2. 値を与えるパターン (他の言語のswitch/matchに相当)
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(
            "
            main(): int {
              cond a {
                10 => {
                  hh: int = 100
                }
                20 => {
                  hh: int = 200
                }
                else => {
                  a: int = 10
                }
              }
            }
            ",
        );
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::Match {
                pattern: Some(Box::new(ex(ExprNode::Var("a".to_string())))),
                arms: vec![
                    MatchArm {
                        pattern: Box::new(ex(ExprNode::Number("10".to_string()))),
                        body: vec![gen_var_node("hh", "100", "int", 5),],
                    },
                    MatchArm {
                        pattern: Box::new(ex(ExprNode::Number("20".to_string()))),
                        body: vec![gen_var_node("hh", "200", "int", 8),],
                    }
                ],
                arm_else: Some(vec![gen_var_node("a", "10", "int", 11)]),
            })
            .wrap_group2()
        );
    }

    #[test]
    fn check_ptr_member_assign() {
        // ポインタが指す構造体のメンバーへの代入: `[ptr].name = 10`
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { [ptr].name = 10 }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::Assign(AssignVar {
                name: "ptr".to_string(),
                dst: Box::new(ex(ExprNode::PtrMember {
                    name: "ptr".to_string(),
                    index: None,
                    else_idx: None,
                    target: Box::new(ex(ExprNode::Var("name".to_string()))),
                })),
                value: Box::new(ex(ExprNode::Number("10".to_string()))),
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_ptr_member_call() {
        // ポインタが指す構造体のメゾットの呼び出し: `[ptr].f()`
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { [ptr].f() }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::PtrMember {
                name: "ptr".to_string(),
                index: None,
                else_idx: None,
                target: Box::new(ex(ExprNode::CallFunc(CallInfo {
                    name: "f".to_string(),
                    temp_ty: Vec::new(),
                    args: Vec::new(),
                }))),
            })
            .wrap_group2()
        );
    }

    #[test]
    fn check_ptr_member_value() {
        // 式の中でポインタが指す構造体のメンバーを読み取る:
        // `a: int = [ptr].name`
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a: int = [ptr].name }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::PtrMember {
                    name: "ptr".to_string(),
                    index: None,
                    else_idx: None,
                    target: Box::new(ex(ExprNode::Var("name".to_string()))),
                })),
                ty: TyNode::Ty("int".to_string()),
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_cast_expr() {
        // キャスト: `(T)var` -> `Cast((T, Box(var)))`
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: i64 = (i64)b ret 1 }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::Cast((
                    TyNode::Ty("i64".to_string()),
                    Box::new(ex(ExprNode::Var("b".to_string()))),
                )))),
                ty: TyNode::Ty("i64".to_string()),
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_enum_variant_expr() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: Color = Color::Green }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::EnumVariant {
                    name: "Color".to_string(),
                    variant: "Green".to_string(),
                })),
                ty: TyNode::Ty("Color".to_string()),
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_postfix_increment_node() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a++ }");
        let Group1Node::FuncDefine(ref func) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            func.body[0].get_node(),
            &ex(ExprNode::Inc(Box::new(ex(ExprNode::Var(
                "a".to_string()
            )))))
            .wrap_group2()
        );
    }

    #[test]
    fn check_postfix_decrement_node() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a-- }");
        let Group1Node::FuncDefine(ref func) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            func.body[0].get_node(),
            &ex(ExprNode::Dec(Box::new(ex(ExprNode::Var(
                "a".to_string()
            )))))
            .wrap_group2()
        );
    }

    #[test]
    fn check_postfix_decrement_after_nested_block() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(
            "main(): int { loop a > 0 { cond { a > 5 => { break } else => { break } } a-- } }",
        );
        assert!(p.parser_for_test(&tkns).is_ok());
    }

    fn parse_value_of(src: &str) -> Expr {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(src);
        let Group1Node::FuncDefine(ref func) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        let Group2Node::Expr(Expr {
            node: ExprNode::DefVar(var),
            ..
        }) = func.body[0].get_node()
        else {
            panic!("not variable definition");
        };
        *var.value.clone()
    }

    fn var(name: &str) -> Box<Expr> {
        Box::new(ex(ExprNode::Var(name.to_string())))
    }

    #[test]
    fn check_unary_sign() {
        assert_eq!(
            parse_value_of("main(): int { a: int = -b }"),
            ex(ExprNode::Sign((false, var("b"))))
        );
        assert_eq!(
            parse_value_of("main(): int { a: int = +b }"),
            ex(ExprNode::Sign((true, var("b"))))
        );
    }

    #[test]
    fn check_unary_not_and_inversion() {
        assert_eq!(
            parse_value_of("main(): int { a: int = !b }"),
            ex(ExprNode::CmpNot(var("b")))
        );
        assert_eq!(
            parse_value_of("main(): int { a: int = ~b }"),
            ex(ExprNode::Inversion(var("b")))
        );
    }

    #[test]
    fn check_unary_prefix_inc_dec() {
        assert_eq!(
            parse_value_of("main(): int { a: int = ++b }"),
            ex(ExprNode::LeftInc(var("b")))
        );
        assert_eq!(
            parse_value_of("main(): int { a: int = --b }"),
            ex(ExprNode::LeftDec(var("b")))
        );
    }

    #[test]
    fn check_unary_nested() {
        // `!~b` / `- -b`
        assert_eq!(
            parse_value_of("main(): int { a: int = !~b }"),
            ex(ExprNode::CmpNot(Box::new(ex(
                ExprNode::Inversion(var("b"))
            ))))
        );
        assert_eq!(
            parse_value_of("main(): int { a: int = - -b }"),
            ex(ExprNode::Sign((
                false,
                Box::new(ex(ExprNode::Sign((false, var("b")))))
            )))
        );
    }

    #[test]
    fn check_unary_precedence() {
        // `-b * c` は `(-b) * c`
        assert_eq!(
            parse_value_of("main(): int { a: int = -b * c }"),
            ex(ExprNode::Mul((
                Box::new(ex(ExprNode::Sign((false, var("b"))))),
                var("c")
            )))
        );
        // `b - -c` は 二項の`-`と単項の`-`
        assert_eq!(
            parse_value_of("main(): int { a: int = b - -c }"),
            ex(ExprNode::Sub((
                var("b"),
                Box::new(ex(ExprNode::Sign((false, var("c")))))
            )))
        );
        // `-(b + c)`
        assert_eq!(
            parse_value_of("main(): int { a: int = -(b + c) }"),
            ex(ExprNode::Sign((
                false,
                Box::new(ex(ExprNode::Add((var("b"), var("c")))))
            )))
        );
    }

    #[test]
    fn check_logical_and_or() {
        assert_eq!(
            parse_value_of("main(): int { a: int = b && c }"),
            ex(ExprNode::CmpAnd((var("b"), var("c"))))
        );
        assert_eq!(
            parse_value_of("main(): int { a: int = b || c }"),
            ex(ExprNode::CmpOr((var("b"), var("c"))))
        );
    }

    #[test]
    fn check_logical_precedence() {
        // `b || c && d` は `b || (c && d)`
        assert_eq!(
            parse_value_of("main(): int { a: int = b || c && d }"),
            ex(ExprNode::CmpOr((
                var("b"),
                Box::new(ex(ExprNode::CmpAnd((var("c"), var("d")))))
            )))
        );
        // `b && c | d` は `b && (c | d)`(ビット演算の方が優先)
        assert_eq!(
            parse_value_of("main(): int { a: int = b && c | d }"),
            ex(ExprNode::CmpAnd((
                var("b"),
                Box::new(ex(ExprNode::OrCalcul((var("c"), var("d")))))
            )))
        );
        // `!b && c` は `(!b) && c`
        assert_eq!(
            parse_value_of("main(): int { a: int = !b && c }"),
            ex(ExprNode::CmpAnd((
                Box::new(ex(ExprNode::CmpNot(var("b")))),
                var("c")
            )))
        );
        // 左結合: `b && c && d` は `(b && c) && d`
        assert_eq!(
            parse_value_of("main(): int { a: int = b && c && d }"),
            ex(ExprNode::CmpAnd((
                Box::new(ex(ExprNode::CmpAnd((var("b"), var("c"))))),
                var("d")
            )))
        );
    }

    #[test]
    fn check_ternary() {
        assert_eq!(
            parse_value_of("main(): int { a: int = b ? c : d }"),
            ex(ExprNode::Ternary((var("b"), var("c"), var("d"))))
        );
    }

    #[test]
    fn check_ternary_precedence() {
        // 条件部は`||`より低い: `b || c ? d : e` は `(b || c) ? d : e`
        assert_eq!(
            parse_value_of("main(): int { a: int = b || c ? d : e }"),
            ex(ExprNode::Ternary((
                Box::new(ex(ExprNode::CmpOr((var("b"), var("c"))))),
                var("d"),
                var("e")
            )))
        );
        // 右結合: `b ? c : d ? e : f` は `b ? c : (d ? e : f)`
        assert_eq!(
            parse_value_of("main(): int { a: int = b ? c : d ? e : f }"),
            ex(ExprNode::Ternary((
                var("b"),
                var("c"),
                Box::new(ex(ExprNode::Ternary((var("d"), var("e"), var("f")))))
            )))
        );
        // then側のネスト: `b ? c ? d : e : f`
        assert_eq!(
            parse_value_of("main(): int { a: int = b ? c ? d : e : f }"),
            ex(ExprNode::Ternary((
                var("b"),
                Box::new(ex(ExprNode::Ternary((var("c"), var("d"), var("e"))))),
                var("f")
            )))
        );
        // 括弧内でも使える
        assert_eq!(
            parse_value_of("main(): int { a: int = (b ? c : d) + e }"),
            ex(ExprNode::Add((
                Box::new(ex(ExprNode::Ternary((var("b"), var("c"), var("d"))))),
                var("e")
            )))
        );
    }
}
