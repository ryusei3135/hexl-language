use crate::compiler::parse::{self, VarMutAttr};

use super::*;
use crate::compiler::ir::result_mod::*;
use err_factory::expr_node;

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

    fn get_ast_len(&self, ast: &node::Expr) -> Option<usize> {
        match ast {
            node::Expr::Str(value) => Some(value.len()),
            node::Expr::Array(values) => Some(values.len()),
            _ => None,
        }
    }

    /// `gen_expr_ir`とほぼ同じ処理を行うが、式が`Point.new()`のような
    /// 構造体を返すスコープ呼び出し(`node::Expr::Scope`)だった場合、
    /// この式の結果を格納する予定の変数名(`var_name`)を`scope_node`に
    /// そのまま渡す。これにより、構造体の初期化のために確保する
    /// 一時的なスタック領域が、コンパイラ内部の合成名(`__self_N`)
    /// ではなく、`var_name`(元の変数名)で`var_tree`に登録される
    /// (詳細は`src/ir/builder/scope.rs`の`scope_node`を参照)。
    fn gen_named_expr_ir<M: ResultMod>(
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
    pub(super) fn infer_expr_ty(&self, expr: &node::Expr) -> Option<types::Size> {
        let ty = match expr {
            node::Expr::Var(name) => {
                let ty_node = self.var_tree.get_ty_node(name)?;
                self.try_size_or_emit_struct(&ty_node).ok()?
            }
            node::Expr::Inc(target) | node::Expr::Dec(target) => {
                return self.infer_expr_ty(target);
            }
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
    pub(super) fn operand_expect_tys(
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
    pub(super) fn resolve_expect_byte<M: ResultMod>(
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

    pub(super) fn build_inc_dec_expr<M: ResultMod>(
        &mut self,
        target: node::Expr,
        kind: inst::ExprKind,
    ) -> Result<inst::Inst, err::ErrKind> {
        let node::Expr::Var(name) = &target else {
            return Err(err::ErrKind::UnexpectedToken);
        };

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

        let idx = self.gen_expr_ir::<M>(target, Some(&ty))?;
        Ok(inst::ExprInst {
            dst: self.id_counter,
            ls: idx,
            rs: idx,
            kind,
        }
        .new())
    }

    pub(super) fn assign_expr_node<M: ResultMod>(
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

    pub(super) fn init_array_node<M: ResultMod>(
        &mut self,
        init_nodes: Vec<node::Expr>,
        expect_byte: &types::Size,
    ) -> Result<inst::Inst, err::ErrKind> {
        let size = match expect_byte {
            types::Size::Array { size, .. } => size,
            _ => panic!("配列の初期化には配列型が必要です"),
        };
        let mut dsts = Vec::new();
        for node in init_nodes.iter() {
            let dst = self.gen_expr_ir::<M>(node.clone(), Some(&size))?;
            dsts.push(dst);
        }
        Ok(inst::Inst::InitArr(dsts))
    }

    #[inline(always)]
    pub(super) fn ref_array_node<M: ResultMod>(
        &mut self,
        dst: node::Expr,
        index: node::Expr,
        name: &str,
        expect_byte: &types::Size,
    ) -> Result<inst::Inst, err::ErrKind> {
        // 添字の範囲チェックは`gen_expr_ir`の`check_mem_access`で行う
        let index_ty = self.index_expect_ty(&index, expect_byte);
        Ok(inst::Inst::InsertArr {
            name: name.to_string(),
            dst: self.gen_expr_ir::<M>(dst, Some(&expect_byte))?,
            index: self.gen_expr_ir::<M>(index, Some(&index_ty))?,
        })
    }

    pub(super) fn def_var_node<M: ResultMod>(
        &mut self,
        var: node::DefineVar,
        expect_byte: &types::Size,
        var_attr: &parse::VarMutAttr,
    ) -> Result<inst::Inst, err::ErrKind> {
        // `register_ty`を指定しない = `var.ty`をそのまま登録する
        self.def_var_node_with_register_ty::<M>(var, expect_byte, var_attr, None)
    }

    /// `def_var_node`の本体
    ///
    /// `register_ty`: `var_tree`/`constract_flag`へ登録する型を、
    /// `var.ty`(値を生成するために実際に使う型)とは別に指定したい場合に渡す。
    /// `must`/`of`の契約付き変数は、値の生成自体は契約を外した中身の型
    /// (`constract.unwrap_ty()`)で行う必要があるが、登録する型は
    /// 契約情報を含む元の型でなければならない。そのため契約付きの分岐から
    /// 中身の型で再帰呼び出しする際に、ここへ元の型(`constract_ty`)を渡し、
    /// 「値の生成には中身の型を使いつつ、登録は元の型のまま行う」を実現する。
    /// `None`の場合は`var.ty`がそのまま登録される
    fn def_var_node_with_register_ty<M: ResultMod>(
        &mut self,
        mut var: node::DefineVar,
        _expect_byte: &types::Size,
        var_attr: &parse::VarMutAttr,
        register_ty: Option<node::TyNode>,
    ) -> Result<inst::Inst, err::ErrKind> {
        match &var.ty.clone() {
            node::TyNode::ConstractMust(constract) | node::TyNode::ConstractOf(constract) => {
                // let _constract_ty = var.ty.clone();
                let mut inner_var = var.clone();
                inner_var.ty = constract.unwrap_ty();

                let value_idx = self.gen_named_expr_ir::<M>(
                    &var.name,
                    *var.value,
                    &self.size_of(&var.ty),
                    &var.var_attr,
                )?;
                // 登録する型: `register_ty`が指定されていればそちらを、
                // 無ければ`var.ty`をそのまま使う(加工しない)
                let ty_to_register = register_ty.as_ref().unwrap_or(&var.ty);
                let _ = self.var_tree.push::<'l'>(
                    &var.name,
                    self.id_counter,
                    ty_to_register,
                    &var_attr,
                    || self.constract_flag.put_var_def(ty_to_register),
                )?;
                let inst = inst::Inst::Mov {
                    name: Some(mem::take(&mut var.name)),
                    size: self.size_of(&constract.unwrap_ty()),
                    dst: self.id_counter,
                    src: value_idx,
                };
                return Ok(inst);
            }
            node::TyNode::Stack { len, .. } => {
                if *len == 1 {
                    // 確保するスタックを増やす
                    self.stack_counter(&var.ty);
                    self.gen_mem_def_var::<M>(var)
                } else {
                    let array_ty = self.size_of(&var.ty);
                    self.stack_counter(&var.ty);
                    let init_values = match *var.value {
                        node::Expr::Array(values) => values,
                        value => vec![value],
                    };
                    let value_idx =
                        self.gen_expr_ir::<M>(node::Expr::Array(init_values), Some(&array_ty))?;
                    let ty_to_register = register_ty.as_ref().unwrap_or(&var.ty);
                    self.var_tree.push::<'l'>(
                        &var.name,
                        self.id_counter,
                        ty_to_register,
                        &var_attr,
                        || self.constract_flag.put_var_def(ty_to_register),
                    )?;
                    Ok(inst::Inst::Mov {
                        name: Some(mem::take(&mut var.name)),
                        size: array_ty,
                        dst: self.id_counter,
                        src: value_idx,
                    })
                }
            }
            node::TyNode::Static { .. } => self.gen_mem_def_var::<M>(var),
            node::TyNode::Ty(ty_name) => {
                // 静的に分かる初期値であれば、以降の範囲チェックで
                // 使えるよう記録しておく(`src/ir/checker/tracking.rs`)
                self.record_var_value(&var.name, &var.value);

                let value_idx = self.gen_named_expr_ir::<M>(
                    &var.name,
                    *var.value,
                    &self.size_of(&var.ty),
                    &var.var_attr,
                )?;
                // 登録する型: `register_ty`が指定されていればそちらを、
                // 無ければ`var.ty`をそのまま使う(加工しない)
                let ty_to_register = register_ty.as_ref().unwrap_or(&var.ty);
                let _ = self.var_tree.push::<'l'>(
                    &var.name,
                    self.id_counter,
                    ty_to_register,
                    &var_attr,
                    || self.constract_flag.put_var_def(ty_to_register),
                )?;
                let inst = inst::Inst::Mov {
                    name: Some(mem::take(&mut var.name)),
                    size: self.size_of(&node::TyNode::Ty(ty_name.to_string())),
                    dst: self.id_counter,
                    src: value_idx,
                };
                return Ok(inst);
            }
            node::TyNode::Pointer { ty_name, range, .. } => {
                let mut r = range.clone();
                // `TyNode::Ty`と同じ理由で、`var.name`をそのまま
                // `gen_named_expr_ir`に渡す(詳細は上のコメントを参照)
                let val = *var.value;
                // 実際の長さ: 配列/文字列リテラルであればここで取得できるが、
                // それ以外の初期値(`0`のような数値など)の場合は`None`になる。
                // `var_tree`への登録後、そちらから長さを取得する(下記参照)
                let literal_len = self.get_ast_len(&val);

                // 範囲付きポインタの初期値が静的に分かる場合、以降の
                // 再代入チェック(`range_ptr_reassign_checker`)などで
                // 使えるよう記録しておく(`src/ir/checker/tracking.rs`)
                self.record_var_value(&var.name, &val);

                let value_idx =
                    self.gen_named_expr_ir::<M>(&var.name, val, &self.size_of(&var.ty), &var_attr)?;
                // 登録する型: `register_ty`が指定されていればそちらを、
                // 無ければ`var.ty`をそのまま使う(加工しない)
                let ty_to_register = register_ty.as_ref().unwrap_or(&var.ty);
                // `TyNode::Ty`と同じ理由で、`Mov`自身のindexを登録する
                // (詳細は上の`TyNode::Ty`分岐のコメントを参照)
                let _ = self.var_tree.push::<'l'>(
                    &var.name,
                    self.id_counter,
                    ty_to_register,
                    &var_attr,
                    || self.constract_flag.put_var_def(ty_to_register),
                )?;

                // リテラルから長さが取れなかった場合は、たった今`var_tree`へ
                // 登録した型(=宣言時に指定された範囲)から長さを取得する
                // (`src/ir/checker/tracking.rs`)。リテラルでも範囲指定でも
                // 長さが分からない場合はここでpanicする
                let base_range =
                    literal_len.unwrap_or_else(|| self.range_len_from_var_tree(&var.name));

                if r.is_none() {
                    r = Some((0, base_range));
                } else {
                    // ポインタの範囲指定がある場合、指定された範囲と初期値の長さが一致するか確認する
                    if r.unwrap() != (0, base_range) {
                        CompileErr::ptr_range_len_mismatch(r.unwrap(), base_range)
                            .map_err(|err| err.with_span(self.current_span))?;
                    }
                }
                let inst = inst::Inst::Mov {
                    name: Some(mem::take(&mut var.name)),
                    size: types::Size::build_ptr_ty(self.size_of(&*ty_name), r),
                    dst: self.id_counter,
                    src: value_idx,
                };
                return Ok(inst);
            }
            t => panic!("{:?}", t),
        }
    }

    /// `Name::Mem`: バリアント型のメンバーへのアクセス
    /// - 全メンバーが型なし(旧列挙型相当)なら、タグを整数値として返す
    /// - それ以外は、型なしメンバーを選んだタグ付きの値を返す
    pub(super) fn enum_variant_node(
        &mut self,
        name: &str,
        variant: &str,
        expect_byte: &types::Size,
    ) -> Result<Box<inst::Inst>, err::ErrKind> {
        let name = self.resolve_variant_name(name, expect_byte);
        let variant_def = self
            .variant_tree
            .get(&name)
            .cloned()
            .ok_or_else(|| expr_node::this_variant_is_undefined(&name).unwrap_err())?;

        if variant_def.is_plain_enum() {
            let tag = variant_def
                .tag_of(variant)
                .ok_or_else(|| expr_node::this_variant_field_is_undefined(variant).unwrap_err())?;
            return Ok(Box::new(inst::Inst::gen_num(
                &tag.to_string(),
                &expect_byte,
                self.id_counter,
            )));
        }
        Ok(Box::new(self.typeless_variant_node(&name, variant)?))
    }

    fn resolve_variant_name(&self, name: &str, expect_byte: &types::Size) -> String {
        if self.variant_tree.contains_key(name) {
            return name.to_string();
        }

        if let types::Size::Variant {
            name: expected_name,
            ..
        } = expect_byte
        {
            if expected_name
                .strip_prefix(name)
                .is_some_and(|suffix| suffix.starts_with('$'))
                && self.variant_tree.contains_key(expected_name)
            {
                return expected_name.clone();
            }
        }

        name.to_string()
    }

    pub(super) fn init_struct_node<M: ResultMod>(
        &mut self,
        name: &str,
        fields: &mut HashMap<String, Box<node::Expr>>,
    ) -> Result<inst::Inst, err::ErrKind> {
        // 構造体のメゾットを処理中かつ初期化する構造体が`self`
        let struct_name: String = if self.this_is_self {
            self.var_tree.get_ty_name(name)?.to_owned()
        } else {
            name.to_string()
        };
        let struct_def = self
            .struct_tree
            .get(&struct_name)
            .cloned()
            .ok_or_else(|| expr_node::this_struct_is_undefined(&struct_name).unwrap_err())?;

        let mut mem_insts = Vec::with_capacity(struct_def.fields.len());
        // フィールドは構造体で定義された順番通りに展開する
        for field in struct_def.fields.clone().iter() {
            let field_size = self.size_of(&field.ty);
            // 確保するスタックを増やす
            self.stack_counter(&field.ty);

            let field_expr = fields.remove(&field.name).unwrap_or_else(|| {
                panic!(
                    "構造体 `{}` の初期化にフィールド `{}` の値がありません",
                    name, field.name
                )
            });

            let value_idx = self.gen_expr_ir::<M>(*field_expr, Some(&field_size))?;
            mem_insts.push(inst::MemoryInst::Member {
                parent: field.name.clone(),
                value_idx: value_idx,
                size: field_size,
            });
        }
        Ok(inst::Inst::Struct {
            name: name.to_string(),
            mem: mem_insts,
            is_self: { self.this_is_self && self.var_tree.is_self_ty(&name) },
        })
    }

    /// `Variant::Mem`(型なしメンバー)からバリアント型の値を作る
    fn typeless_variant_node(
        &mut self,
        variant_name: &str,
        field_name: &str,
    ) -> Result<inst::Inst, err::ErrKind> {
        let variant_def = self.variant_tree.get_variant(variant_name)?;
        let tag = variant_def
            .tag_of(field_name)
            .ok_or_else(|| expr_node::this_variant_field_is_undefined(field_name).unwrap_err())?;
        if variant_def.fields[tag].ty.is_some() {
            panic!(
                "バリアント型 `{}` のメンバー `{}` は値が必要です(`{} {{ {}: 値 }}`)",
                variant_name, field_name, variant_name, field_name
            );
        }

        // バリアント型全体のスタックを確保する
        self.stack_counter(&node::TyNode::Ty(variant_name.to_string()));
        Ok(inst::Inst::Variant {
            name: variant_name.to_string(),
            tag,
            value: None,
            tagged: variant_def.is_tagged(),
            size: self.size_of_variant(&variant_def),
            is_self: false,
        })
    }

    /// バリアント型の初期化: `Name { field: value }`
    /// バリアント型は同時に1つのメンバーしか持てないので、指定できるのは1つだけ
    pub(super) fn init_variant_node<M: ResultMod>(
        &mut self,
        is_self: bool,
        name: &str,
        fields: &mut HashMap<String, Box<node::Expr>>,
        expect_byte: &types::Size,
    ) -> Result<inst::Inst, err::ErrKind> {
        // メゾットの処理中、初期化するバリアント型が`self`の場合は
        // 変数の型から実際のバリアント型の名前を求める
        let resolve_self = is_self && self.this_is_self;
        let variant_name: String = if resolve_self {
            self.var_tree.get_ty_name(name)?.to_owned()
        } else {
            self.resolve_variant_name(name, expect_byte)
        };
        let variant_def = self.variant_tree.get_variant(&variant_name)?;

        if fields.len() != 1 {
            panic!(
                "バリアント型 `{}` の初期化では、メンバーをちょうど1つ指定してください({}個指定されています)",
                variant_name,
                fields.len()
            );
        }
        let (field_name, field_expr) = fields.drain().next().unwrap();
        let tag = variant_def
            .tag_of(&field_name)
            .ok_or_else(|| expr_node::this_variant_field_is_undefined(&field_name).unwrap_err())?;

        // バリアント型全体のスタックを一度だけ確保する
        self.stack_counter(&node::TyNode::Ty(variant_name.clone()));

        let field_ty = variant_def.fields[tag].ty.clone();
        let value = match field_ty {
            Some(ty) => {
                let field_size = self.size_of(&ty);
                let value_idx = self.gen_expr_ir::<M>(*field_expr, Some(&field_size))?;
                Some(inst::MemoryInst::Member {
                    parent: field_name,
                    value_idx,
                    size: field_size,
                })
            }
            None => panic!(
                "バリアント型 `{}` のメンバー `{}` は型がないので値を渡せません",
                variant_name, field_name
            ),
        };

        Ok(inst::Inst::Variant {
            name: variant_name,
            tag,
            value,
            tagged: variant_def.is_tagged(),
            size: self.size_of_variant(&variant_def),
            is_self: resolve_self && self.var_tree.is_self_ty(name),
        })
    }
}
