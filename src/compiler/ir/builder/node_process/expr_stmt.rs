//! 式の処理: 型の推論、二項/単項演算、キャスト、`++`/`--`、代入

use crate::compiler::ast::Expr;

use super::*;
use crate::compiler::ast::{AssignVar, DefineVar, ExprNode, TyNode};

impl IR {
    fn ensure_var_is_assignable(
        &self,
        name: &str,
        attr: &parse::VarMutAttr,
    ) -> Result<(), err::ErrKind> {
        // `must`の契約を持つ変数は、値をすり替えられないため再代入できない
        if self.var_tree.is_contract_must(name) {
            CompileErr::assign_to_must_var(name, Rc::clone(&self.current_span))?;
        }
        if !matches!(attr, parse::VarMutAttr::Var) {
            CompileErr::assign_to_imm_var(name, Rc::clone(&self.current_span))?;
        }
        Ok(())
    }

    /// `gen_expr_ir`とほぼ同じ処理を行うが、式が`Point.new()`のような
    /// 構造体を返すスコープ呼び出し(`Expr::Scope`)だった場合、
    /// この式の結果を格納する予定の変数名(`var_name`)を`scope_node`に
    /// そのまま渡す。これにより、構造体の初期化のために確保する
    /// 一時的なスタック領域が、コンパイラ内部の合成名(`__self_N`)
    /// ではなく、`var_name`(元の変数名)で`var_tree`に登録される
    /// (詳細は`src/ir/builder/scope.rs`の`scope_node`を参照)。
    pub(super) fn gen_named_expr_ir<M: ResultMod>(
        &mut self,
        var_name: &str,
        expr: Expr,
        expect_byte: &types::Size,
        attr: &VarMutAttr,
    ) -> Result<usize, err::ErrKind> {
        // `Type::Mem(value)`がバリアント型の初期化なら、`InitVariant`に直す
        // (`src/ir/builder/variant_bind.rs`)
        let expr = self.scope_call_to_variant_init(&expr);
        if let ExprNode::Scope { scope, target } = &expr.node {
            self.expr_counter += 1;
            let result = self.scope_node::<M>(scope, target, Some(&var_name.to_string()), &attr);
            let inst = self.unwrap_or_report(result);
            self.ir_tree.push(inst);
            self.id_counter += 1;
            Ok(self.id_counter - 1)
        } else if let ExprNode::CallFunc(call) = &expr.node {
            let result = self.gen_call_fn_ir::<M>(None, call, Some(&var_name.to_owned()));
            let inst = self.unwrap_or_report(result);
            self.ir_tree.push(inst);
            self.id_counter += 1;
            Ok(self.id_counter - 1)
        } else {
            self.gen_expr_ir::<M>(&expr, Some(expect_byte))
        }
    }

    /// 配列の添字に渡す、期待する型
    ///
    /// 添字は配列の要素(`expect_byte`)とは別の型でよいので、添字が変数なら
    /// その変数の型(`uint`など)にする。型が分からない(即値など)ときは、
    /// 整数の`expect_byte`をそのまま使い、整数でなければ`DD`にする
    pub(in crate::compiler::ir) fn index_expect_ty(
        &self,
        index: &Expr,
        expect_byte: &types::Size,
    ) -> types::Size {
        match self.infer_expr_ty(index) {
            Some(ty) if !ty.is_float() => ty,
            _ if expect_byte.is_float() => types::Size::DD,
            _ => expect_byte.clone(),
        }
    }

    /// 型が決まっていない即値(`10`や`1.5`、即値だけの式`1 + 2`)か
    fn is_untyped_literal(expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::Number(_) | ExprNode::Float(_) => true,
            ExprNode::Add(n)
            | ExprNode::Sub(n)
            | ExprNode::Mul(n)
            | ExprNode::Div(n)
            | ExprNode::Surplus(n)
            | ExprNode::LShift(n)
            | ExprNode::RShift(n)
            | ExprNode::AndCalcul(n)
            | ExprNode::XorCalcul(n)
            | ExprNode::OrCalcul(n) => {
                Self::is_untyped_literal(&n.0) && Self::is_untyped_literal(&n.1)
            }
            // `-1`や`~0`のような、即値だけの単項演算
            ExprNode::Sign((_, target))
            | ExprNode::Inversion(target)
            | ExprNode::CmpNot(target) => Self::is_untyped_literal(&target),
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
    pub(in super::super) fn infer_expr_ty(&self, expr: &Expr) -> Option<types::Size> {
        let ty = match &expr.node {
            ExprNode::Var(name) => {
                let ty_node = self.var_tree.get_ty_node(name)?;
                self.try_size_or_emit_struct(&ty_node).ok()?
            }
            ExprNode::Inc(target) | ExprNode::Dec(target) => {
                return self.infer_expr_ty(target);
            }
            // 単項演算の結果は、オペランドと同じ型
            // (`!a`は比較の結果なので、型を決める元にしない)
            ExprNode::Sign((_, target))
            | ExprNode::Inversion(target)
            | ExprNode::LeftInc(target)
            | ExprNode::LeftDec(target) => {
                return self.infer_expr_ty(target);
            }
            // キャストの結果は、キャスト先の型(組み込み型のみ)
            ExprNode::Cast((ty, _)) => match ty {
                TyNode::Ty(_) => types::Size::try_from(ty).ok()?,
                _ => return None,
            },
            ExprNode::Add(n)
            | ExprNode::Sub(n)
            | ExprNode::Mul(n)
            | ExprNode::Div(n)
            | ExprNode::Surplus(n)
            | ExprNode::LShift(n)
            | ExprNode::RShift(n)
            | ExprNode::AndCalcul(n)
            | ExprNode::XorCalcul(n)
            | ExprNode::OrCalcul(n) => self
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
        ls: &Expr,
        rs: &Expr,
        expect_byte: &types::Size,
    ) -> (types::Size, types::Size) {
        let fit = |lit: &Expr, other: &Expr| -> Option<types::Size> {
            if Self::is_untyped_literal(lit) == false || Self::is_untyped_literal(other) {
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
    fn literal_has_float(expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::Float(_) => true,
            ExprNode::Add(n)
            | ExprNode::Sub(n)
            | ExprNode::Mul(n)
            | ExprNode::Div(n)
            | ExprNode::Surplus(n)
            | ExprNode::LShift(n)
            | ExprNode::RShift(n)
            | ExprNode::AndCalcul(n)
            | ExprNode::XorCalcul(n)
            | ExprNode::OrCalcul(n) => {
                Self::literal_has_float(&n.0) || Self::literal_has_float(&n.1)
            }
            ExprNode::Sign((_, target)) => Self::literal_has_float(target),
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
        expr: &Expr,
        expect_byte: Option<&types::Size>,
    ) -> Result<types::Size, err::ErrKind> {
        if let ExprNode::Assign(assign_node) = &expr.node {
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
                match expr.node {
                    ExprNode::Number(_) | ExprNode::Float(_) => None,
                    // 比較(`n % 15 == 0`/`i < 10`)は、結果ではなくオペランドの型で決める
                    ExprNode::LessThen(ref n)
                    | ExprNode::GreaterThen(ref n)
                    | ExprNode::LessThenEq(ref n)
                    | ExprNode::GreaterThenEq(ref n)
                    | ExprNode::Equal(ref n)
                    | ExprNode::NotEq(ref n) => self
                        .infer_expr_ty(&n.0)
                        .or_else(|| self.infer_expr_ty(&n.1)),
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
        dst: &Expr,
        expect_byte: Option<&types::Size>,
    ) -> Result<Option<types::Size>, err::ErrKind> {
        // (構造体名, メンバー名, 配列メンバーの要素か)
        let (struct_name, member, is_arr_elem) = match &dst.node {
            ExprNode::Member { scope, target } => {
                // スコープがemptyでないことは確定している
                let var_name = scope.last().unwrap();
                let struct_name = M::handle(self.var_tree.get_ty_name(var_name).or_else(|_| {
                    Err(crate::GenUndefErrResult!(
                        UndefStruct,
                        var_name.to_string(),
                        None
                    ))
                }))?;
                match &target.node {
                    ExprNode::Var(m) => (struct_name, m.clone(), false),
                    ExprNode::RefArray { name, .. } => (struct_name, name.clone(), true),
                    // パーサーですでにはじいているのでpanic
                    n => panic!("構造体のメンバーに使えないノード {:?}", n),
                }
            }
            ExprNode::PtrMember { name, target, .. } => {
                // // 代入先のメンバーの型(`member_ty_of_dst`は副作用なしの版)
                // let Some(member_ty) = self.member_ty_of_dst(dst) else {
                //     return Err();
                // };

                if let Some(guard) = self.check_mem_access::<M>(dst).unwrap() {
                    // 添字が可変で、範囲内と証明できない: 実行時ガード付きで格納する
                    self.insert_arr_guard::<M>(guard, &dst, &expect_byte.unwrap())?;
                }

                let struct_name = self.var_tree.get_ty_name(name)?;
                match &target.node {
                    ExprNode::Var(m) => (struct_name, m.clone(), false),
                    ExprNode::RefArray { name, .. } => (struct_name, name.clone(), true),
                    // パーサーですでにはじいているのでpanic
                    n => panic!("構造体のメンバーに使えないノード {:?}", n),
                }
            }
            // これは呼び出し元が`resolve_expect_byte`で代入先がメンバーかを調べているので、Noneっを返す
            _ => return Ok(None),
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

    /// `Expr::Cast((ty, target))`を`inst::Inst::Cast`にする
    ///
    /// キャスト先`ty`は組み込みの型(`byte`/`int`/`u64`/`float`など)に限る。
    /// 構造体・バリアント型・ポインタ・配列などは受け付けない
    ///
    /// - 組み込み型の名前でない(ユーザー定義の型名など): `TryFrom`が返す未定義の型のエラー
    /// - `TyNode::Ty`以外の型: `UnexpectedToken`
    ///   (専用のエラーを`err`に用意したら差し替える)
    pub(in super::super) fn build_cast_inst<M: ResultMod>(
        &mut self,
        ty: &TyNode,
        target: &Expr,
    ) -> Result<inst::Inst, err::ErrKind> {
        let after = match ty {
            TyNode::Ty(_) => types::Size::try_from(ty)?,
            _ => return Err(err::ErrKind::UnexpectedToken),
        };
        // 変換元は、式自身の型で生成する(分からなければキャスト先の型)
        let before = self.infer_expr_ty(target).unwrap_or_else(|| after.clone());
        let src = self.gen_expr_ir::<M>(target, Some(&before))?;
        Ok(inst::Inst::Cast { src, after })
    }

    /// `++`/`--`できる型(整数型)か
    fn is_inc_dec_integer(ty: &types::Size) -> bool {
        matches!(
            ty,
            types::Size::DB
                | types::Size::UDB
                | types::Size::DW
                | types::Size::UDW
                | types::Size::DD
                | types::Size::UDD
                | types::Size::DQ
                | types::Size::UDQ
        )
    }

    /// `++`/`--`(前置/後置)の対象の変数を確認して、その型を返す
    ///
    /// 対象は、再代入できる整数型の変数のみ
    /// (構造体のメンバーは`inc_dec_member_check`で確認する)
    ///
    /// `name`は`var_tree`に登録された変数の名前に限る。メンバーの名前(`len`など)は
    /// 変数として登録されていないので、呼び出し側は`inc_dec_var_ty`に対象の式ごと渡す
    fn inc_dec_name_ty<M: ResultMod>(&mut self, name: &str) -> Result<types::Size, err::ErrKind> {
        let var_attr = self.var_tree.is_mut(name);
        self.ensure_var_is_assignable(name, &var_attr)?;

        let ty_node = self.var_tree.get_ty_node(name).unwrap();
        let ty = M::handle(self.try_size_or_emit_struct(&ty_node))?;
        if !Self::is_inc_dec_integer(&ty) {
            return Err(CompileErr::inc_dec_requires_integer(
                name,
                Rc::clone(&self.current_span),
            ));
        }
        Ok(ty)
    }

    /// `++`/`--`の対象が構造体のメンバーなら、再代入できる整数型のメンバーかを
    /// 確認して、メンバーを持つ変数(代入ノードの`name`)の名前を返す
    ///
    /// - `self->len` / `[ptr].len`(`PtrMember`): 変数は、ポインタ変数(`self`など)
    /// - `a.len`(`Member`): 変数は、構造体の変数`a`
    ///
    /// 検査の内容は代入(`assign_expr_node`)と同じ
    /// (`must`の契約を持つメンバー、不変な変数は再代入できない)
    ///
    /// 対象がメンバーではない(変数など)なら`Ok(None)`
    fn inc_dec_member_check<M: ResultMod>(
        &mut self,
        target: &Expr,
    ) -> Result<Option<(String, types::Size)>, err::ErrKind> {
        // (メンバーを持つ変数, メンバー名, ポインタ経由か)
        let (owner, member, via_ptr) = match &target.node {
            ExprNode::PtrMember { name, target, .. } => match &target.node {
                ExprNode::Var(member) => (name.clone(), member.clone(), true),
                _ => return Err(err::ErrKind::UnexpectedToken),
            },
            ExprNode::Member { scope, target } => {
                let Some(owner) = scope.last() else {
                    return Err(err::ErrKind::UnexpectedToken);
                };
                match &target.node {
                    ExprNode::Var(member) => (owner.clone(), member.clone(), false),
                    _ => return Err(err::ErrKind::UnexpectedToken),
                }
            }
            _ => return Ok(None),
        };

        let var_attr = self.var_tree.is_mut(&owner);
        if via_ptr {
            // `assign_expr_node`の`PtrMember`の代入と同じ検査
            if !self.in_constructor()
                && self
                    .ptr_member_field_ty(&owner, &member)
                    .is_some_and(|ty| ty.is_contract_must())
            {
                CompileErr::assign_to_must_var(&member, Rc::clone(&self.current_span))?;
            }
            if !matches!(var_attr, parse::VarMutAttr::Var) {
                CompileErr::assign_to_imm_var(&member, Rc::clone(&self.current_span))?;
            }
        } else {
            self.ensure_var_is_assignable(&owner, &var_attr)?;
        }

        // メンバーの型。`get_ty_name`は、ポインタ型の変数には指す先の構造体名を返す
        let struct_name = self.var_tree.get_ty_name(&owner)?;
        let field_ty = self
            .struct_tree
            .get(&struct_name)
            .ok_or_else(|| crate::GenUndefErrResult!(UndefStruct, struct_name.to_string(), None))?
            .fields
            .iter()
            .find(|field| field.name == member)
            .ok_or_else(|| crate::GenUndefErrResult!(UndefStructMember, member.to_string(), None))?
            .ty
            .clone();
        let ty = M::handle(self.try_size_or_emit_struct(&field_ty))?;
        if !Self::is_inc_dec_integer(&ty) {
            return Err(CompileErr::inc_dec_requires_integer(
                &member,
                Rc::clone(&self.current_span),
            ));
        }
        Ok(Some((owner, ty)))
    }

    /// `++`/`--`(前置/後置)の対象の式を確認して、その型を返す
    ///
    /// - 変数(`a`): 再代入できる整数型の変数
    /// - 構造体のメンバー(`self->len` / `[ptr].len` / `a.len`): 構造体の定義からメンバーの
    ///   型を調べる。メンバーの名前だけを変数として調べない(`len`は変数ではない)
    ///
    /// 戻り値は、(代入ノードの`name`になる変数の名前, 対象の型)
    fn inc_dec_var_ty<M: ResultMod>(
        &mut self,
        target: &Expr,
    ) -> Result<(String, types::Size), err::ErrKind> {
        match &target.node {
            ExprNode::Var(name) => {
                let ty = self.inc_dec_name_ty::<M>(name)?;
                Ok((name.clone(), ty))
            }
            _ => self
                .inc_dec_member_check::<M>(target)?
                .ok_or(err::ErrKind::UnexpectedToken),
        }
    }

    /// `++a` / `a++`などを、代入`a = a + 1`(`a = a - 1`)のノードに展開する
    ///
    /// 対象は、変数、または構造体のメンバー(`self->len` / `[ptr].len` / `a.len`)。
    /// メンバーは、読み出したメモリ上の値を増減しても元に書き戻されないため、
    /// 代入に展開してメンバーへ書き込む。式の値は、増減後の値になる
    fn desugar_inc_dec<M: ResultMod>(
        &mut self,
        expr: &Expr,
        target: &Expr,
        is_inc: bool,
    ) -> Result<Expr, err::ErrKind> {
        let span = Rc::clone(&expr.span);
        // 代入ノードの`name`になる、変数の名前(対象の式ごと確認する)
        let (owner, _) = self.inc_dec_var_ty::<M>(target)?;
        let calc = Expr::wrap(
            target.clone(),
            ExprNode::Number("1".to_string()).with_span(Rc::clone(&span)),
        );
        let value = if is_inc {
            ExprNode::Add(calc)
        } else {
            ExprNode::Sub(calc)
        };
        Ok(AssignVar::new(
            &owner,
            target.clone(),
            value.with_span(Rc::clone(&span)),
            span,
        ))
    }

    pub(in super::super) fn build_inc_dec_expr<M: ResultMod>(
        &mut self,
        target: &Expr,
        kind: &inst::ExprKind,
    ) -> Result<inst::Inst, err::ErrKind> {
        // 変数のみ(メンバーは`desugar_unary`が代入に展開済み)
        if !matches!(&target.node, ExprNode::Var(_)) {
            return Err(err::ErrKind::UnexpectedToken);
        }

        let (_, ty) = self.inc_dec_var_ty::<M>(target)?;

        let idx = self.gen_expr_ir::<M>(target, Some(&ty))?;
        Ok(inst::ExprInst {
            dst: self.id_counter,
            ls: idx,
            rs: idx,
            kind: kind.clone(),
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
        cond: &Box<Expr>,
        left_expr: &Box<Expr>,
        right_expr: &Box<Expr>,
        expect_byte: &types::Size,
    ) -> Result<(usize, usize, usize), err::ErrKind> {
        self.ternatry = Some(true);
        let left = self.gen_expr_ir::<M>(&*left_expr, Some(expect_byte))?;
        let right = self.gen_expr_ir::<M>(&*right_expr, Some(expect_byte))?;
        let cond = self.gen_ternary_cond_ir::<M>(&*cond)?;
        Ok((cond, left, right))
    }

    /// `[ptr->member index else else_idx]`(`PtrMember`の`target`が`RefArray`)を、
    /// ポインタを一時変数に入れた通常の配列アクセスに展開する
    ///
    /// ```text
    /// $ptr_arr_N: <メンバーの型> = ptr->member      // ここで`DefVar`のIRを出力する
    /// [$ptr_arr_N index else else_idx]               // 戻り値の式
    /// ```
    ///
    /// ポインタ変数への配列アクセス(`[p index]`)と同じ処理に乗せるための展開で、
    /// `PtrMember`の`RefArray`をそのままIRにすると、添字が捨てられてメンバー自体への
    /// アクセスになってしまう。
    ///
    /// 戻り値は、(一時変数の名前, 配列アクセスの式)
    fn ptr_member_arr_to_tmp<M: ResultMod>(
        &mut self,
        dst: &Expr,
    ) -> Result<(String, Expr), err::ErrKind> {
        let ExprNode::PtrMember {
            name: ptr, target, ..
        } = &dst.node
        else {
            return Err(err::ErrKind::UnexpectedToken);
        };
        let ExprNode::RefArray {
            name: member,
            index,
            else_idx,
            ..
        } = &target.node
        else {
            return Err(err::ErrKind::UnexpectedToken);
        };

        // メンバーの型(ポインタ/配列)。`get_ty_name`は、ポインタ変数には指す先の構造体名を返す
        let struct_name = self.var_tree.get_ty_name(ptr)?;
        let field_ty = self
            .struct_tree
            .get(&struct_name)
            .ok_or_else(|| crate::GenUndefErrResult!(UndefStruct, struct_name.to_string(), None))?
            .fields
            .iter()
            .find(|field| field.name == *member)
            .ok_or_else(|| crate::GenUndefErrResult!(UndefStructMember, member.to_string(), None))?
            .ty
            .clone();
        let field_size = M::handle(self.try_size_or_emit_struct(&field_ty))?;

        // 一時変数は、メンバーの値(ポインタ)のコピーにすぎないので、メンバーの契約
        // (`T* must=free`など)は引き継がない。契約の終了は、構造体を初期化した
        // スコープの`del`で行うもので、一時変数に契約を持たせると、
        // 要素の代入が「`must`変数への再代入」になったり、一時変数の契約終了を
        // 求められたりする
        let tmp_ty = match &field_ty {
            TyNode::ConstractMust(inner) | TyNode::ConstractOf(inner) => inner.unwrap_ty(),
            other => other.clone(),
        };
        // 要素の書き込み可否は、ポインタ変数(`self`など)の可変性に従う
        let tmp_attr = self.var_tree.is_mut(ptr);

        let span = Rc::clone(&dst.span);
        let tmp = format!("$ptr_arr_{}", self.id_counter);
        // `ptr->member`(メンバーそのものの値 = 配列の先頭を指すポインタ)
        let member_value = Expr::new(
            ExprNode::PtrMember {
                name: ptr.clone(),
                index: None,
                else_idx: None,
                target: Box::new(Expr::new(ExprNode::Var(member.clone()), Rc::clone(&span))),
            },
            Rc::clone(&span),
        );
        let def_var = Expr::new(
            ExprNode::DefVar(DefineVar {
                name: tmp.clone(),
                value: Box::new(member_value),
                ty: tmp_ty,
                var_attr: tmp_attr,
            }),
            Rc::clone(&span),
        );
        self.gen_expr_ir::<M>(&def_var, Some(&field_size))?;

        let tmp_var = Expr::new(ExprNode::Var(tmp.clone()), Rc::clone(&span));
        let arr = Expr::new(
            ExprNode::RefArray {
                name: tmp.clone(),
                dst: Box::new(tmp_var),
                index: index.clone(),
                else_idx: else_idx.clone(),
            },
            span,
        );
        Ok((tmp, arr))
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
    /// - `self->len++` / `a.len--`など、メンバーの後置`++`/`--`も、同じく代入に展開する
    ///   (`a`が変数の後置は`build_inc_dec_expr`)
    ///
    /// 単項演算のノードでなければ`Ok(None)`
    pub(in super::super) fn desugar_unary<M: ResultMod>(
        &mut self,
        expr: &Expr,
        expect_byte: Option<&types::Size>,
    ) -> Result<Option<Expr>, err::ErrKind> {
        // オペランドの型。分からなければ呼び出し側の期待する型
        let operand_ty = |this: &Self, target: &Expr| -> Option<types::Size> {
            this.infer_expr_ty(target).or_else(|| expect_byte.cloned())
        };

        let expanded = match &expr.node {
            ExprNode::Sign((true, target)) => (**target).clone(),
            ExprNode::Sign((false, target)) => {
                let is_float = operand_ty(self, target).is_some_and(|ty| ty.is_float())
                    || Self::literal_has_float(target);
                let zero = if is_float {
                    ExprNode::Float("0.0".to_string()).with_span(Rc::clone(&expr.span))
                } else {
                    ExprNode::Number("0".to_string()).with_span(Rc::clone(&expr.span))
                };
                ExprNode::Sub(Expr::wrap(zero, (**target).clone())).with_span(Rc::clone(&expr.span))
            }
            ExprNode::Inversion(target) => {
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
                ExprNode::XorCalcul(Expr::wrap(
                    (**target).clone(),
                    ExprNode::Number(mask).with_span(Rc::clone(&expr.span)),
                ))
                .with_span(Rc::clone(&expr.span))
            }
            ExprNode::CmpNot(target) => ExprNode::Equal(Expr::wrap(
                (**target).clone(),
                ExprNode::Number("0".to_string()).with_span(Rc::clone(&expr.span)),
            ))
            .with_span(Rc::clone(&expr.span)),
            // `++a` / `--a`
            ExprNode::LeftInc(target) | ExprNode::LeftDec(target) => {
                self.desugar_inc_dec::<M>(expr, target, matches!(&expr.node, ExprNode::LeftInc(_)))?
            }
            // 構造体のメンバーの`a->len++` / `a.len--`(変数の`a++`は`build_inc_dec_expr`が処理する)
            ExprNode::Inc(target) | ExprNode::Dec(target)
                if matches!(
                    &target.node,
                    ExprNode::PtrMember { .. } | ExprNode::Member { .. }
                ) =>
            {
                self.desugar_inc_dec::<M>(expr, target, matches!(&expr.node, ExprNode::Inc(_)))?
            }
            // `[ptr->member index] = value`: 一時変数に入れたポインタへの配列の代入に展開する
            ExprNode::Assign(assign)
                if matches!(
                    &assign.dst.node,
                    ExprNode::PtrMember { target, .. }
                        if matches!(target.node, ExprNode::RefArray { .. })
                ) =>
            {
                let (tmp, arr) = self.ptr_member_arr_to_tmp::<M>(&assign.dst)?;
                AssignVar::new(&tmp, arr, (*assign.value).clone(), Rc::clone(&expr.span))
            }
            // `[ptr->member index]`(値の読み取り)
            ExprNode::PtrMember { target, .. }
                if matches!(target.node, ExprNode::RefArray { .. }) =>
            {
                self.ptr_member_arr_to_tmp::<M>(expr)?.1
            }
            _ => return Ok(None),
        };
        Ok(Some(expanded))
    }

    /// `value`が、ポインタ`ptr_name`の指す構造体のメンバー`member`を
    /// 引数に含む関数呼び出し(`f(self->val, ..)` / `mod::f<T>(self->val, ..)`)か
    fn value_hands_over_member(value: &Expr, ptr_name: &str, member: &str) -> bool {
        fn is_member(arg: &Expr, ptr_name: &str, member: &str) -> bool {
            match &arg.node {
                ExprNode::ConnectAddr(target) => is_member(target, ptr_name, member),
                ExprNode::PtrMember { name, target, .. } => {
                    name.as_str() == ptr_name
                        && matches!(&target.node, ExprNode::Var(m) if m.as_str() == member)
                }
                _ => false,
            }
        }

        let call = match &value.node {
            ExprNode::CallFunc(call) => call,
            ExprNode::Scope { target, .. } => match &target.node {
                ExprNode::CallFunc(call) => call,
                _ => return false,
            },
            _ => return false,
        };
        call.args.iter().any(|arg| is_member(arg, ptr_name, member))
    }

    pub(in super::super) fn assign_expr_node<M: ResultMod>(
        &mut self,
        assign_node: &AssignVar,
        expect_byte: &types::Size,
        attr: &parse::VarMutAttr,
    ) -> Result<inst::Inst, err::ErrKind> {
        // `self->name = value` / `[ptr].name = value`の代入先は、ポインタ変数
        // (`assign_node.name`、`self`など)ではなく、指す先の構造体のメンバー`name`。
        // 変数の型や追跡している値は、メンバーではなくポインタ変数のものなので、
        // メンバーへの代入では`name`の型で検査し、ポインタ変数の記録は更新しない
        let ptr_member = match &assign_node.dst.node {
            ExprNode::PtrMember { name, target, .. } => match &target.node {
                ExprNode::Var(member) => Some((name.as_str(), member.as_str())),
                _ => None,
            },
            _ => None,
        };

        let value_name = if let Some((ptr_name, member)) = ptr_member {
            // `must`の契約を持つメンバーは、値をすり替えられないため再代入できない
            // ただし、構造体の初期化(`new`メゾット)の中での最初の代入は許可する。
            // フィールドの契約は、構造体を初期化したスコープで終了すればよい
            // また、`self->val = f(self->val, ..)`のように、そのメンバー自身を
            // 関数へ渡して、戻り値を受け取る代入も許可する(値のすり替えではなく、
            // 契約の譲渡と受け取り)。渡し先が`must=..`の引数かどうかは、
            // 呼び出しの引数のチェック(`check_constract_arg`)が確認する
            if !self.in_constructor()
                && !Self::value_hands_over_member(&assign_node.value, ptr_name, member)
                && self
                    .ptr_member_field_ty(ptr_name, member)
                    .is_some_and(|ty| ty.is_contract_must())
            {
                CompileErr::assign_to_must_var(member, Rc::clone(&self.current_span))?;
            }
            if !matches!(attr, parse::VarMutAttr::Var) {
                CompileErr::assign_to_imm_var(member, Rc::clone(&self.current_span))?;
            }
            // 値の生成で一時領域を確保する場合に、ポインタ変数(`self`)の名前で
            // 変数を登録して上書きしないよう、メンバー用の別名を使う
            format!("$ptr_member_{}", self.id_counter)
        } else {
            self.ensure_var_is_assignable(&assign_node.name, attr)?;

            // 範囲付きポインタ変数への再代入が範囲を超えていないかを確認する
            // (`src/ir/checker/tracking.rs`)
            self.range_ptr_reassign_checker(&assign_node.name, &assign_node.value);
            // 静的に分かる値であれば、以降の範囲チェックで使えるよう記録しておく
            self.record_var_value(&assign_node.name, &assign_node.value);
            assign_node.name.clone()
        };

        let right_expr_idx: usize = self.gen_named_expr_ir::<M>(
            &value_name,
            *assign_node.value.clone(),
            &expect_byte,
            &attr,
        )?;
        let dst_idx = self.gen_expr_ir::<M>(&*assign_node.dst, Some(&expect_byte))?;

        Ok(inst::Inst::AssignVar {
            name: assign_node.name.to_string(),
            dst: dst_idx,
            value: right_expr_idx,
        })
    }

    /// 三項演算子の条件式として使える式か
    ///
    /// - 比較(`<` `>` `<=` `>=` `==` `!=`)・`&&`・`||`・`!`
    /// - 比較の両辺は、メモリにアクセスしない式(`is_mem_free`)だけ
    ///   (配列・ポインタの中身・メンバー・関数呼び出し・代入・`++`/`--`は不可)
    fn is_ternary_cond(expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::CmpAnd(n) | ExprNode::CmpOr(n) => {
                Self::is_ternary_cond(&n.0) && Self::is_ternary_cond(&n.1)
            }
            ExprNode::CmpNot(target) => Self::is_ternary_cond(target),
            _ => Self::is_mem_free(expr),
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
        cond: &Expr,
    ) -> Result<usize, err::ErrKind> {
        if Self::is_ternary_cond(cond) == false {
            panic!(
                "三項演算子の条件式には、ポインタのアクセスや関数呼び出しなどは使えません: {:?}",
                cond
            );
        }

        // 条件式全体の型(`gen_expr_ir`と同じ決め方)
        let size = self.resolve_expect_byte::<M>(cond, None)?;

        match &cond.node {
            // 比較が1つだけ
            ExprNode::LessThen(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryLessThen)
            }
            ExprNode::GreaterThen(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryGreaterThen)
            }
            ExprNode::LessThenEq(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryLessThenEq)
            }
            ExprNode::GreaterThenEq(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryGreaterThenEq)
            }
            ExprNode::Equal(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryEqual)
            }
            ExprNode::NotEq(n) => {
                self.gen_ternary_cmp_ir::<M>(n, &size, inst::ExprKind::TernaryNotEq)
            }

            // `&&` / `||`: 結果(0/1)が0でないか
            ExprNode::CmpAnd(..) | ExprNode::CmpOr(..) => {
                let value = self.gen_setcc_ir::<M>(cond)?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num(
                    &"0".to_string(),
                    &size,
                    self.id_counter,
                ));
                Ok(self.push_ternary_cmp(inst::ExprKind::TernaryNotEq, value, zero))
            }

            // `!(..)`: 結果(0/1)が0か
            ExprNode::CmpNot(target) => {
                let value = self.gen_setcc_ir::<M>(&*target)?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num(
                    &"0".to_string(),
                    &size,
                    self.id_counter,
                ));
                Ok(self.push_ternary_cmp(inst::ExprKind::TernaryEqual, value, zero))
            }

            // 比較ではない式: 0でないか
            _ => {
                let value = self.gen_expr_ir::<M>(cond, Some(&size))?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num(
                    &"0".to_string(),
                    &size,
                    self.id_counter,
                ));
                Ok(self.push_ternary_cmp(inst::ExprKind::TernaryNotEq, value, zero))
            }
        }
    }

    /// 比較1つ分の`Ternary*`命令
    fn gen_ternary_cmp_ir<M: ResultMod>(
        &mut self,
        node: &(Box<Expr>, Box<Expr>),
        size: &types::Size,
        kind: inst::ExprKind,
    ) -> Result<usize, err::ErrKind> {
        // 即値の型は、もう片方のオペランドに合わせる(`left_right_pair`)
        let (ls, rs) = self.left_right_pair::<M>(node, size)?;
        Ok(self.push_ternary_cmp(kind, ls, rs))
    }

    /// `Set*`(setcc)命令で、`expr`を0か1の値にする必要があるか
    ///
    /// 次の場合に`true`になる
    /// - `bool`型への代入・引数・戻り値など、期待する型が`bool`の比較(`b: bool = a < c`)
    /// - 条件式が複雑な場合: `&&` / `||`、または`!`の中が比較・`&&` / `||` / `!`
    /// - 三項演算子の複雑な条件式(`gen_ternary_cond_ir`)
    ///
    /// それ以外の単純な比較(`if a < b`や三項演算子の`a < b ? x : y`)は、
    /// 今までどおり`LessThen`や`Ternary*`を使う
    pub(in crate::compiler::ir) fn needs_setcc(
        expr: &Expr,
        expect_byte: Option<&types::Size>,
    ) -> bool {
        let is_bool = matches!(expect_byte, Some(types::Size::Bool));
        match &expr.node {
            ExprNode::CmpAnd(..) | ExprNode::CmpOr(..) => true,
            ExprNode::CmpNot(target) => {
                is_bool || Self::is_cmp_expr(target) || Self::is_logic_expr(target)
            }
            _ => is_bool && Self::is_cmp_expr(expr),
        }
    }

    /// 比較(`<` `>` `<=` `>=` `==` `!=`)か
    fn is_cmp_expr(expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::LessThen(..)
            | ExprNode::GreaterThen(..)
            | ExprNode::LessThenEq(..)
            | ExprNode::GreaterThenEq(..)
            | ExprNode::Equal(..)
            | ExprNode::NotEq(..) => true,
            _ => false,
        }
    }

    /// `&&` / `||` / `!`か
    fn is_logic_expr(expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::CmpAnd(..) | ExprNode::CmpOr(..) | ExprNode::CmpNot(..) => true,
            _ => false,
        }
    }

    /// 比較のオペランドの型(`Set*`の両辺に渡す、期待する型)
    ///
    /// 変数などオペランドの型が分かればその型、分からなければ`DD`。
    /// `bool`への代入でも、オペランドの型は`bool`にならない(`check_var_ty`で弾かれる)
    fn set_cmp_operand_ty(&self, node: &(Box<Expr>, Box<Expr>)) -> types::Size {
        self.infer_expr_ty(&node.0)
            .or_else(|| self.infer_expr_ty(&node.1))
            .unwrap_or(types::Size::DD)
    }

    /// 比較1つ分の`Set*`命令(結果は0か1)
    fn gen_set_cmp_ir<M: ResultMod>(
        &mut self,
        node: &(Box<Expr>, Box<Expr>),
        kind: inst::ExprKind,
    ) -> Result<usize, err::ErrKind> {
        let size = self.set_cmp_operand_ty(&node);
        let (ls, rs) = self.left_right_pair::<M>(node, &size)?;
        Ok(self.push_ternary_cmp(kind, ls, rs))
    }

    /// 比較・`&&` / `||` / `!`を、0か1の値を作る`Set*`命令へ展開して、その値の番号を返す
    ///
    /// - `a < b`  -> `SetLessThen(a, b)`(`>` `<=` `>=` `==` `!=`も同様に`Set*`)
    /// - `x && y` -> `AndCalcul(<x>, <y>)`
    /// - `x || y` -> `OrCalcul(<x>, <y>)`
    /// - `!x`     -> `SetEqual(<x>, 0)`
    /// - 比較ではない式`x` -> `SetNotEq(x, 0)`(`bool`型の変数は0/1なのでそのまま使う)
    ///
    /// `&&` / `||`は、両辺を必ず評価する(短絡評価はしない)。
    /// 右辺に副作用やメモリアクセスがあるときは、分岐で展開する必要がある
    pub(in crate::compiler::ir) fn gen_setcc_ir<M: ResultMod>(
        &mut self,
        expr: &Expr,
    ) -> Result<usize, err::ErrKind> {
        use inst::ExprKind as K;
        self.expr_counter += 1;
        match &expr.node {
            ExprNode::LessThen(n) => self.gen_set_cmp_ir::<M>(n, K::SetLessThen),
            ExprNode::GreaterThen(n) => self.gen_set_cmp_ir::<M>(n, K::SetGreaterThen),
            ExprNode::LessThenEq(n) => self.gen_set_cmp_ir::<M>(n, K::SetLessThenEq),
            ExprNode::GreaterThenEq(n) => self.gen_set_cmp_ir::<M>(n, K::SetGreaterThenEq),
            ExprNode::Equal(n) => self.gen_set_cmp_ir::<M>(n, K::SetEqual),
            ExprNode::NotEq(n) => self.gen_set_cmp_ir::<M>(n, K::SetNotEq),
            ExprNode::CmpAnd((l, r)) => {
                let ls = self.gen_setcc_ir::<M>(&*l)?;
                let rs = self.gen_setcc_ir::<M>(&*r)?;
                Ok(self.push_ternary_cmp(K::AndCalcul, ls, rs))
            }
            ExprNode::CmpOr((l, r)) => {
                let ls = self.gen_setcc_ir::<M>(&*l)?;
                let rs = self.gen_setcc_ir::<M>(&*r)?;
                Ok(self.push_ternary_cmp(K::OrCalcul, ls, rs))
            }
            ExprNode::CmpNot(target) => {
                let value = self.gen_setcc_ir::<M>(&*target)?;
                let bool_ty = types::Size::Bool;
                let zero =
                    self.push_ternary_inst(inst::Inst::gen_num("0", &bool_ty, self.id_counter));
                Ok(self.push_ternary_cmp(K::SetEqual, value, zero))
            }
            // 比較ではない式
            _ => {
                // `bool`型の変数は、すでに0か1
                if let ExprNode::Var(name) = &expr.node {
                    let is_bool = self
                        .var_tree
                        .get_ty_node(name)
                        .and_then(|ty| self.try_size_or_emit_struct(&ty).ok())
                        .is_some_and(|ty| ty == types::Size::Bool);
                    if is_bool {
                        return self.gen_expr_ir::<M>(&expr, Some(&types::Size::Bool));
                    }
                }
                let size = self.infer_expr_ty(&expr).unwrap_or(types::Size::DD);
                let value = self.gen_expr_ir::<M>(&expr, Some(&size))?;
                let zero = self.push_ternary_inst(inst::Inst::gen_num("0", &size, self.id_counter));
                Ok(self.push_ternary_cmp(K::SetNotEq, value, zero))
            }
        }
    }
}