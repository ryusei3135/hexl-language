//! 式の処理: 型の推論、二項/単項演算、キャスト、`++`/`--`、代入

mod cond_expr_node;
mod prepros_expr;
mod finalize_expr_ty;
mod access_member;

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
}