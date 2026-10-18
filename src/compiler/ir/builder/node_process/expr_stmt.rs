//! 式の処理: 型の推論、二項/単項演算、キャスト、`++`/`--`、代入

use crate::compiler::node::Expr;

use super::*;

impl IR {
    fn ensure_var_is_assignable(
        &self,
        name: &str,
        attr: &parse::VarMutAttr,
    ) -> Result<(), err::ErrKind> {
        // `must`の契約を持つ変数は、値をすり替えられないため再代入できない
        if self.var_tree.is_constract_must(name) {
            CompileErr::assign_to_must_var(name).map_err(|err| err.with_span(self.current_span))?;
        }
        if !matches!(attr, parse::VarMutAttr::Var) {
            CompileErr::assign_to_imm_var(name).map_err(|err| err.with_span(self.current_span))?;
        }
        Ok(())
    }

    /// `gen_expr_ir`とほぼ同じ処理を行うが、式が`Point.new()`のような
    /// 構造体を返すスコープ呼び出し(`node::Expr::Scope`)だった場合、
    /// この式の結果を格納する予定の変数名(`var_name`)を`scope_node`に
    /// そのまま渡す。これにより、構造体の初期化のために確保する
    /// 一時的なスタック領域が、コンパイラ内部の合成名(`__self_N`)
    /// ではなく、`var_name`(元の変数名)で`var_tree`に登録される
    /// (詳細は`src/ir/builder/scope.rs`の`scope_node`を参照)。
    pub(super) fn gen_named_expr_ir<M: ResultMod>(
        &mut self,
        var_name: &str,
        expr: node::Expr,
        expect_byte: &types::Size,
        attr: &VarMutAttr,
    ) -> Result<usize, err::ErrKind> {
        // `Type::Mem(value)`がバリアント型の初期化なら、`InitVariant`に直す
        // (`src/ir/builder/variant_bind.rs`)
        let expr = self.scope_call_to_variant_init(expr);
        if let node::Expr::Scope { scope, target } = expr {
            self.expr_counter += 1;
            let result = self.scope_node::<M>(&scope, target, Some(&var_name.to_string()), &attr);
            let inst = self.unwrap_or_report(result);
            self.ir_tree.push(inst);
            self.id_counter += 1;
            Ok(self.id_counter - 1)
        } else if let node::Expr::CallFunc(call) = expr {
            let result = self.gen_call_fn_ir::<M>(None, &call, Some(&var_name.to_owned()));
            let inst = self.unwrap_or_report(result);
            self.ir_tree.push(inst);
            self.id_counter += 1;
            Ok(self.id_counter - 1)
        } else {
            self.gen_expr_ir::<M>(expr, Some(expect_byte))
        }
    }

    /// 配列の添字に渡す、期待する型
    ///
    /// 添字は配列の要素(`expect_byte`)とは別の型でよいので、添字が変数なら
    /// その変数の型(`uint`など)にする。型が分からない(即値など)ときは、
    /// 整数の`expect_byte`をそのまま使い、整数でなければ`DD`にする
    pub(in crate::compiler::ir) fn index_expect_ty(
        &self,
        index: &node::Expr,
        expect_byte: &types::Size,
    ) -> types::Size {
        match self.infer_expr_ty(index) {
            Some(ty) if !ty.is_float() => ty,
            _ if expect_byte.is_float() => types::Size::DD,
            _ => expect_byte.clone(),
        }
    }

    /// 型が決まっていない即値(`10`や`1.5`、即値だけの式`1 + 2`)か
    fn is_untyped_literal(expr: &node::Expr) -> bool {
        match expr {
            node::Expr::Number(_) | node::Expr::Float(_) => true,
            node::Expr::Add(n)
            | node::Expr::Sub(n)
            | node::Expr::Mul(n)
            | node::Expr::Div(n)
            | node::Expr::Surplus(n)
            | node::Expr::LShift(n)
            | node::Expr::RShift(n)
            | node::Expr::AndCalcul(n)
            | node::Expr::XorCalcul(n)
            | node::Expr::OrCalcul(n) => {
                Self::is_untyped_literal(&n.0) && Self::is_untyped_literal(&n.1)
            }
            // `-1`や`~0`のような、即値だけの単項演算
            node::Expr::Sign((_, target))
            | node::Expr::Inversion(target)
            | node::Expr::CmpNot(target) => Self::is_untyped_literal(target),
            _ => false,
        }
    }

    /// 即値の型を決める元になる、式の型(定義元の型)を調べる
    ///
    /// - 変数は、定義した型
    /// - 二項演算は、型が分かるほうのオペランドの型
    /// - 数値型(整数・浮動小数点)だけを返す。ポインタ・構造体などは
    ///   即値の型にできないので`None`
    ///
    /// 副作用が無い(変数の使用回数も数えない)ので、`&self`で調べる
    pub(in super::super) fn infer_expr_ty(&self, expr: &node::Expr) -> Option<types::Size> {
        let ty = match expr {
            node::Expr::Var(name) => {
                let ty_node = self.var_tree.get_ty_node(name)?;
                self.try_size_or_emit_struct(&ty_node).ok()?
            }
            node::Expr::Inc(target) | node::Expr::Dec(target) => {
                return self.infer_expr_ty(target);
            }
            // 単項演算の結果は、オペランドと同じ型
            // (`!a`は比較の結果なので、型を決める元にしない)
            node::Expr::Sign((_, target))
            | node::Expr::Inversion(target)
            | node::Expr::LeftInc(target)
            | node::Expr::LeftDec(target) => {
                return self.infer_expr_ty(target);
            }
            // キャストの結果は、キャスト先の型(組み込み型のみ)
            node::Expr::Cast((ty, _)) => match ty {
                node::TyNode::Ty(_) => types::Size::try_from(ty).ok()?,
                _ => return None,
            },
            node::Expr::Add(n)
            | node::Expr::Sub(n)
            | node::Expr::Mul(n)
            | node::Expr::Div(n)
            | node::Expr::Surplus(n)
            | node::Expr::LShift(n)
            | node::Expr::RShift(n)
            | node::Expr::AndCalcul(n)
            | node::Expr::XorCalcul(n)
            | node::Expr::OrCalcul(n) => self
                .infer_expr_ty(&n.0)
                .or_else(|| self.infer_expr_ty(&n.1))?,
            _ => return None,
        };
        use types::Size;
        matches!(
            ty,
            Size::DB
                | Size::DW
                | Size::DD
                | Size::DQ
                | Size::UDB
                | Size::UDW
                | Size::UDD
                | Size::UDQ
                | Size::F32
                | Size::F64
        )
        .then_some(ty)
    }

    /// 二項演算の左右のオペランドに渡す、期待する型を決める
    ///
    /// 片方が型の決まっていない即値で、もう片方の型が分かるなら、
    /// 即値の側はもう片方の型(定義元の型)にする
    /// (`a: uint64`に対する`a < 10`や`10 < a`の`10`は`uint64`になる)。
    /// 整数の即値は整数型へ、小数の即値は浮動小数点型へだけ変換する。
    /// それ以外は、渡された`expect_byte`をそのまま使う
    pub(in super::super) fn operand_expect_tys(
        &self,
        ls: &node::Expr,
        rs: &node::Expr,
        expect_byte: &types::Size,
    ) -> (types::Size, types::Size) {
        let fit = |lit: &node::Expr, other: &node::Expr| -> Option<types::Size> {
            if !Self::is_untyped_literal(lit) || Self::is_untyped_literal(other) {
                return None;
            }
            let ty = self.infer_expr_ty(other)?;
            let lit_is_float = Self::literal_has_float(lit);
            (ty.is_float() == lit_is_float).then_some(ty)
        };
        let l = fit(ls, rs).unwrap_or_else(|| expect_byte.clone());
        let r = fit(rs, ls).unwrap_or_else(|| expect_byte.clone());
        (l, r)
    }

    /// 即値の式の中に小数(`Expr::Float`)があるか
    fn literal_has_float(expr: &node::Expr) -> bool {
        match expr {
            node::Expr::Float(_) => true,
            node::Expr::Add(n)
            | node::Expr::Sub(n)
            | node::Expr::Mul(n)
            | node::Expr::Div(n)
            | node::Expr::Surplus(n)
            | node::Expr::LShift(n)
            | node::Expr::RShift(n)
            | node::Expr::AndCalcul(n)
            | node::Expr::XorCalcul(n)
            | node::Expr::OrCalcul(n) => {
                Self::literal_has_float(&n.0) || Self::literal_has_float(&n.1)
            }
            node::Expr::Sign((_, target)) => Self::literal_has_float(target),
            _ => false,
        }
    }

    /// `gen_expr_ir`に渡された期待する型を確定させる
    ///
    /// - `Assign`ノードなら、代入先の型で確定させる
    ///   - 構造体のメンバー(`var.member` / `[ptr].member` /
    ///     `var.[member idx]`)への代入なら、そのメンバーの型
    ///   - それ以外は、代入先の変数の型
    /// - それ以外は、渡された型をそのまま使う
    /// - どちらも無い(`None`で`Assign`でもない)なら、二項演算は
    ///   オペランドの変数の型(定義元の型)、それ以外は`DD`にする
    pub(in super::super) fn resolve_expect_byte<M: ResultMod>(
        &mut self,
        expr: &node::Expr,
        expect_byte: Option<&types::Size>,
    ) -> Result<types::Size, err::ErrKind> {
        if let node::Expr::Assign(assign_node) = expr {
            if let Some(size) = self.member_ty_of_assign_dst::<M>(&assign_node.dst, expect_byte)? {
                return Ok(size);
            }
            if let Some(ty_node) = self.var_tree.get_ty_node(&assign_node.name) {
                let result = self.try_size_or_emit_struct(&ty_node);
                return Ok(self.unwrap_or_report(result));
            }
        }
        Ok(expect_byte
            .cloned()
            .or_else(|| {
                // 型が決まっていない二項演算(`if a < 10`など)は、
                // オペランドの型で決める
                match expr {
                    node::Expr::Number(_) | node::Expr::Float(_) => None,
                    _ => self.infer_expr_ty(expr),
                }
            })
            .unwrap_or(types::Size::DD))
    }

    /// 代入先が構造体のメンバーなら、そのメンバーの型を返す
    /// # Panics
    /// - メンバーのノードが変数以外ならpanic
    fn member_ty_of_assign_dst<M: ResultMod>(
        &mut self,
        dst: &node::Expr,
        expect_byte: Option<&types::Size>,
    ) -> Result<Option<types::Size>, err::ErrKind> {
        // (構造体名, メンバー名, 配列メンバーの要素か)
        let (struct_name, member, is_arr_elem) = match dst {
            node::Expr::Member { scope, target } => {
                // スコープがemptyでないことは確定している
                let var_name = scope.last().unwrap();
                let struct_name = M::handle(self.var_tree.get_ty_name(var_name).or_else(|_| {
                    Err(crate::GenUndefErrResult!(
                        UndefStruct,
                        var_name.to_string(),
                        None
                    ))
                }))?;
                match &**target {
                    node::Expr::Var(m) => (struct_name, m.clone(), false),
                    node::Expr::RefArray { name, .. } => (struct_name, name.clone(), true),
                    // パーサーですでにはじいているのでpanic
                    n => panic!("構造体のメンバーに使えないノード {:?}", n),
                }
            }
            node::Expr::PtrMember { name, target, .. } => {
                // // 代入先のメンバーの型(`member_ty_of_dst`は副作用なしの版)
                // let Some(member_ty) = self.member_ty_of_dst(dst) else {
                //     return Err();
                // };

                match self.check_mem_access::<M>(dst).unwrap() {
                    // 添字が可変で、範囲内と証明できない: 実行時ガード付きで格納する
                    Some(guard) => {
                        let name = name.clone();
                        self.insert_arr_guard::<M>(guard, dst.clone(), &expect_byte.unwrap())?;

                        let struct_name = self.var_tree.get_ty_name(&name)?;
                        match &**target {
                            node::Expr::Var(m) => (struct_name, m.clone(), false),
                            node::Expr::RefArray { name, .. } => (struct_name, name.clone(), true),
                            // パーサーですでにはじいているのでpanic
                            n => panic!("構造体のメンバーに使えないノード {:?}", n),
                        }
                    }
                    // 添字なし、または静的に範囲内と分かった場合: 今までの経路
                    None => {
                        panic!();
                    }
                }
            }
            // これは呼び出し元が`resolve_expect_byte`で代入先がメンバーかを調べているので、Noneっを返す
            n => return Ok(None),
        };
        if is_arr_elem {
            // 配列メンバーは要素1つ分の型にする(`member_is_arr_ref`と同じ)
            let field_ty = self
                .struct_tree
                .get(&struct_name)
                .ok_or_else(|| {
                    crate::GenUndefErrResult!(UndefStruct, struct_name.to_string(), None)
                })?
                .fields
                .iter()
                .find(|field| field.name == member)
                .ok_or_else(|| {
                    crate::GenUndefErrResult!(UndefStructMember, member.to_string(), None)
                })?
                .ty
                .clone();
            self.try_size_or_emit_struct(&field_ty).map(|x| Some(x))
        } else {
            Ok(Some(self.struct_tree.get_mem_size(&struct_name, &member)))
        }
    }

    /// `node::Expr::Cast((ty, target))`を`inst::Inst::Cast`にする
    ///
    /// キャスト先`ty`は組み込みの型(`byte`/`int`/`u64`/`float`など)に限る。
    /// 構造体・バリアント型・ポインタ・配列などは受け付けない
    ///
    /// - 組み込み型の名前でない(ユーザー定義の型名など): `TryFrom`が返す未定義の型のエラー
    /// - `TyNode::Ty`以外の型: `UnexpectedToken`
    ///   (専用のエラーを`err`に用意したら差し替える)
    pub(in super::super) fn build_cast_inst<M: ResultMod>(
        &mut self,
        ty: &node::TyNode,
        target: node::Expr,
    ) -> Result<inst::Inst, err::ErrKind> {
        let after = match ty {
            node::TyNode::Ty(_) => types::Size::try_from(ty)?,
            _ => return Err(err::ErrKind::UnexpectedToken),
        };
        // 変換元は、式自身の型で生成する(分からなければキャスト先の型)
        let before = self.infer_expr_ty(&target).unwrap_or_else(|| after.clone());
        let src = self.gen_expr_ir::<M>(target, Some(&before))?;
        Ok(inst::Inst::Cast { src, after })
    }

    /// `++`/`--`(前置/後置)の対象の変数を確認して、その型を返す
    ///
    /// 対象は、再代入できる整数型の変数のみ
    fn inc_dec_var_ty<M: ResultMod>(&mut self, name: &str) -> Result<types::Size, err::ErrKind> {
        let var_attr = self.var_tree.is_mut(name);
        self.ensure_var_is_assignable(name, &var_attr)?;

        let ty_node = self.var_tree.get_ty_node(name).unwrap();
        let ty = M::handle(self.try_size_or_emit_struct(&ty_node))?;
        if !matches!(
            &ty,
            types::Size::DB
                | types::Size::UDB
                | types::Size::DW
                | types::Size::UDW
                | types::Size::DD
                | types::Size::UDD
                | types::Size::DQ
                | types::Size::UDQ
        ) {
            return Err(CompileErr::inc_dec_requires_integer(name).with_span(self.current_span));
        }
        Ok(ty)
    }

    pub(in super::super) fn build_inc_dec_expr<M: ResultMod>(
        &mut self,
        target: node::Expr,
        kind: inst::ExprKind,
    ) -> Result<inst::Inst, err::ErrKind> {
        let node::Expr::Var(name) = &target else {
            return Err(err::ErrKind::UnexpectedToken);
        };

        let ty = self.inc_dec_var_ty::<M>(name)?;

        let idx = self.gen_expr_ir::<M>(target, Some(&ty))?;
        Ok(inst::ExprInst {
            dst: self.id_counter,
            ls: idx,
            rs: idx,
            kind,
        }
        .new())
    }

    /// 三項演算子を処理する
    /// 中身の式によって普通の条件分岐に展開する
    /// 
    /// ## Tarnaryノードに変換する条件
    /// - 中身が数字か数字のみの式
    /// - 条件式がメモリにアクセスしていない
    /// - ternaryがSome(trueなら三項演算子が生成される)
    pub(in super::super) fn ternary_expr<M: ResultMod>(
        &mut self,
        cond: Box<Expr>,
        left_expr: Box<Expr>, 
        right_expr: Box<Expr>,
        expect_byte: &types::Size,
    ) -> Result<(usize, usize, usize), err::ErrKind> {
        self.ternatry = Some(true);
        let left = self.gen_expr_ir::<M>(*left_expr, Some(expect_byte))?;
        let right = self.gen_expr_ir::<M>(*right_expr, Some(expect_byte))?;
        let cond = self.gen_ternary_cond_ir::<M>(*cond)?;
        Ok((cond, left, right))
    }

    /// 前置の単項演算子(`+` `-` `!` `~` `++` `--`)を、既存のノードに展開する
    ///
    /// 単項演算専用の命令は作らず、バックエンドが対応済みの命令だけで表す
    /// - `+a`  -> `a`
    /// - `-a`  -> `0 - a`
    /// - `~a`  -> `a ^ (全ビットが1の値)`
    /// - `!a`  -> `a == 0`
    /// - `++a` -> `a = a + 1`(式の値は、加算後の値)
    /// - `--a` -> `a = a - 1`(式の値は、減算後の値)
    ///
    /// 単項演算のノードでなければ`Ok(None)`
    pub(in super::super) fn desugar_unary<M: ResultMod>(
        &mut self,
        expr: &node::Expr,
        expect_byte: Option<&types::Size>,
    ) -> Result<Option<node::Expr>, err::ErrKind> {
        // オペランドの型。分からなければ呼び出し側の期待する型
        let operand_ty = |this: &Self, target: &node::Expr| -> Option<types::Size> {
            this.infer_expr_ty(target).or_else(|| expect_byte.cloned())
        };
        let num = |value: &str| node::Expr::Number(value.to_string());

        Ok(Some(match expr {
            node::Expr::Sign((true, target)) => (**target).clone(),
            node::Expr::Sign((false, target)) => {
                let is_float = operand_ty(self, target).is_some_and(|ty| ty.is_float())
                    || Self::literal_has_float(target);
                let zero = if is_float {
                    node::Expr::Float("0.0".to_string())
                } else {
                    num("0")
                };
                node::Expr::Sub(node::Expr::wrap(zero, (**target).clone()))
            }
            node::Expr::Inversion(target) => {
                use types::Size;
                // 全ビットが1の値(符号付きは-1、符号なしは型の最大値)
                let mask = match operand_ty(self, target).unwrap_or(Size::DD) {
                    Size::UDB => u8::MAX.to_string(),
                    Size::UDW => u16::MAX.to_string(),
                    Size::UDD => u32::MAX.to_string(),
                    Size::UDQ => u64::MAX.to_string(),
                    Size::F32 | Size::F64 => return Err(err::ErrKind::UnexpectedToken),
                    _ => "-1".to_string(),
                };
                node::Expr::XorCalcul(node::Expr::wrap((**target).clone(), num(&mask)))
            }
            node::Expr::CmpNot(target) => {
                node::Expr::Equal(node::Expr::wrap((**target).clone(), num("0")))
            }
            node::Expr::LeftInc(target) | node::Expr::LeftDec(target) => {
                let node::Expr::Var(name) = &**target else {
                    return Err(err::ErrKind::UnexpectedToken);
                };
                // 後置の`++`/`--`と同じ条件(再代入できる整数型の変数)を確認する
                self.inc_dec_var_ty::<M>(name)?;
                let var = node::Expr::Var(name.clone());
                let calc = node::Expr::wrap(var.clone(), num("1"));
                let value = if matches!(expr, node::Expr::LeftInc(_)) {
                    node::Expr::Add(calc)
                } else {
                    node::Expr::Sub(calc)
                };
                node::AssignVar::new(name, var, value)
            }
            _ => return Ok(None),
        }))
    }

    pub(in super::super) fn assign_expr_node<M: ResultMod>(
        &mut self,
        assign_node: node::AssignVar,
        expect_byte: &types::Size,
        attr: &parse::VarMutAttr,
    ) -> Result<inst::Inst, err::ErrKind> {
        self.ensure_var_is_assignable(&assign_node.name, attr)?;

        // 範囲付きポインタ変数への再代入が範囲を超えていないかを確認する
        // (`src/ir/checker/tracking.rs`)
        self.range_ptr_reassign_checker(&assign_node.name, &assign_node.value);
        // 静的に分かる値であれば、以降の範囲チェックで使えるよう記録しておく
        self.record_var_value(&assign_node.name, &assign_node.value);

        let right_expr_idx: usize = self.gen_named_expr_ir::<M>(
            &assign_node.name,
            *assign_node.value,
            &expect_byte,
            &attr,
        )?;
        let dst_idx = self.gen_expr_ir::<M>(*assign_node.dst, Some(&expect_byte))?;

        Ok(inst::Inst::AssignVar {
            name: assign_node.name.to_string(),
            dst: dst_idx,
            value: right_expr_idx,
        })
    }

    // builder.rs の `impl IR { .. }` の中(`gen_ternary_branch_ir`の下など)に貼り付ける
    //
    // ※ `&&` / `||` のノード名は、このファイルからは分からなかったので
    //   `node::Expr::LogicalAnd` / `node::Expr::LogicalOr`(どちらも `(Box<Expr>, Box<Expr>)`)
    //   と仮定している。実際の名前に置き換えること。

    /// 三項演算子の条件式として使える式か
    ///
    /// - 比較(`<` `>` `<=` `>=` `==` `!=`)・`&&`・`||`・`!`
    /// - 比較の両辺は、メモリにアクセスしない式(`is_mem_free`)だけ
    ///   (配列・ポインタの中身・メンバー・関数呼び出し・代入・`++`/`--`は不可)
    fn is_ternary_cond(expr: &node::Expr) -> bool {
        match expr {
            node::Expr::CmpAnd(n) | node::Expr::CmpOr(n) => {
                Self::is_ternary_cond(&n.0) && Self::is_ternary_cond(&n.1)
            }
            node::Expr::CmpNot(target) => Self::is_ternary_cond(target),
            other => Self::is_mem_free(other),
        }
    }

    /// 生成した命令を`ir_tree`へ積み、その命令の番号を返す
    fn push_ternary_inst(&mut self, inst: inst::Inst) -> usize {
        self.ir_tree.push(inst);
        self.id_counter += 1;
        self.id_counter - 1
    }

    /// `ExprKind::Ternary*`(比較の結果を`Ternary`が使うための命令)を作る
    fn push_ternary_cmp(&mut self, kind: inst::ExprKind, ls: usize, rs: usize) -> usize {
        let inst = inst::ExprInst {
            dst: self.id_counter,
            ls,
            rs,
            kind,
        }
        .new();
        self.push_ternary_inst(inst)
    }

    /// 三項演算子の条件式から、`ExprKind::Ternary*`の命令を生成する
    /// 戻り値は、生成した`Ternary*`命令の番号
    ///
    /// - 比較が1つだけ(`a < b` `a != b` など)
    ///   → その比較に対応する`TernaryLessThen` `TernaryGreaterThen`
    ///     `TernaryLessThenEq` `TernaryGreaterThenEq` `TernaryEqual` `TernaryNotEq`を1命令だけ生成
    /// - `&&` / `||` がある(`a < b && c != d` など)
    ///   → 各比較を通常の比較(0/1)で作り、`&&`は`AndCalcul`、`||`は`OrCalcul`で畳み込んでから
    ///     `TernaryNotEq(結果, 0)`を生成する
    ///   → `!(..)`は`TernaryEqual(結果, 0)`を生成する
    /// - 比較ではない式(`x` `x + 1`など)は`TernaryNotEq(式, 0)`
    ///
    /// ポインタのアクセス・関数呼び出しなどを含む条件式は受け付けない(panic)。
    /// 呼び出し側は先に`is_ternary_cond`で確認すること。
    pub(in crate::compiler::ir) fn gen_ternary_cond_ir<M: ResultMod>(
        &mut self,
        cond: node::Expr,
    ) -> Result<usize, err::ErrKind> {
        if !Self::is_ternary_cond(&cond) {
            panic!(
                "三項演算子の条件式には、ポインタのアクセスや関数呼び出しなどは使えません: {:?}",
                cond
            );
        }

        // 条件式全体の型(`gen_expr_ir`と同じ決め方)
        let size = self.resolve_expect_byte::<M>(&cond, None)?;

        match cond {
            // 比較が1つだけ
            node::Expr::LessThen(n) => self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryLessThen),
            node::Expr::GreaterThen(n) => self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryGreaterThen),
            node::Expr::LessThenEq(n) => self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryLessThenEq),
            node::Expr::GreaterThenEq(n) => self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryGreaterThenEq),
            node::Expr::Equal(n) => self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryEqual),
            node::Expr::NotEq(n) => self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryNotEq),

            // `&&` / `||`: 結果(0/1)が0でないか
            and_or @ (node::Expr::CmpAnd(..) | node::Expr::CmpOr(..)) => {
                let value = self.gen_ternary_bool_ir::<M>(and_or, &size)?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num(&"0".to_string(), &size, self.id_counter));
                Ok(self.push_ternary_cmp(inst::ExprKind::TernaryNotEq, value, zero))
            }

            // `!(..)`: 結果(0/1)が0か
            node::Expr::CmpNot(target) => {
                let value = self.gen_ternary_bool_ir::<M>(*target, &size)?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num(&"0".to_string(), &size, self.id_counter));
                Ok(self.push_ternary_cmp(inst::ExprKind::TernaryEqual, value, zero))
            }

            // 比較ではない式: 0でないか
            other => {
                let value = self.gen_expr_ir::<M>(other, Some(&size))?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num(&"0".to_string(), &size, self.id_counter));
                Ok(self.push_ternary_cmp(inst::ExprKind::TernaryNotEq, value, zero))
            }
        }
    }

    /// 比較1つ分の`Ternary*`命令
    fn gen_ternary_cmp_ir<M: ResultMod>(
        &mut self,
        node: (Box<node::Expr>, Box<node::Expr>),
        size: &types::Size,
        kind: inst::ExprKind,
    ) -> Result<usize, err::ErrKind> {
        // 即値の型は、もう片方のオペランドに合わせる(`left_right_pair`)
        let (ls, rs) = self.left_right_pair::<M>(node, size)?;
        Ok(self.push_ternary_cmp(kind, ls, rs))
    }

    /// `&&` / `||` / `!` / 比較を、0か1の値を作る通常の命令へ展開して、その値の番号を返す
    /// (`Ternary*`は使わない。`gen_ternary_cond_ir`が最後に1回だけ生成する)
    fn gen_ternary_bool_ir<M: ResultMod>(
        &mut self,
        expr: node::Expr,
        size: &types::Size,
    ) -> Result<usize, err::ErrKind> {
        match expr {
            node::Expr::CmpAnd((l, r)) => {
                let ls = self.gen_ternary_bool_ir::<M>(*l, size)?;
                let rs = self.gen_ternary_bool_ir::<M>(*r, size)?;
                Ok(self.push_ternary_cmp(inst::ExprKind::AndCalcul, ls, rs))
            }
            node::Expr::CmpOr((l, r)) => {
                let ls = self.gen_ternary_bool_ir::<M>(*l, size)?;
                let rs = self.gen_ternary_bool_ir::<M>(*r, size)?;
                Ok(self.push_ternary_cmp(inst::ExprKind::OrCalcul, ls, rs))
            }
            // `!x`は、既存の単項演算子の展開(`== 0`)に任せる
            // 比較は通常の`LessThen`などが0/1を作る
            other => self.gen_expr_ir::<M>(other, Some(size)),
        }
    }
}
