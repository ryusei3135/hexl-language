use crate::{
    err::PreprocErrs::NotFoundAsmName,
    parse::{self, VarMutAttr},
};

use super::*;

impl IR {
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
    fn gen_named_expr_ir(
        &mut self,
        var_name: &String,
        expr: node::Expr,
        expect_byte: &types::Size,
        attr: &VarMutAttr,
    ) -> usize {
        if let node::Expr::Scope { scope, target } = expr {
            self.expr_counter += 1;
            let result = self.scope_node(&scope, target, Some(var_name), &attr);
            let inst = self.unwrap_or_report(result);
            self.ir_tree.push(inst);
            self.id_counter += 1;
            self.id_counter - 1
        } else if let node::Expr::CallFunc(call) = expr {
            let result = self.gen_call_fn_ir(None, &call, Some(var_name));
            let inst = self.unwrap_or_report(result);
            self.ir_tree.push(inst);
            self.id_counter += 1;
            self.id_counter - 1
        } else {
            self.gen_expr_ir(expr, expect_byte)
        }
    }

    pub(super) fn assign_expr_node(
        &mut self,
        assign_node: node::AssignVar,
        expect_byte: &types::Size,
        attr: &parse::VarMutAttr,
    ) -> Result<inst::Inst, err::ErrKind> {
        // `must`の契約を持つ変数は、`mut`かどうかに関わらず
        // 再代入できない(契約が別の値にすり替わってしまうため)
        if self.var_tree.is_constract_must(&assign_node.name) {
            CompileErr::assign_to_must_var(&assign_node.name)
                .map_err(|err| err.with_span(self.current_span))?;
        }

        if !matches!(attr, parse::VarMutAttr::Var) {
            CompileErr::assign_to_imm_var(&assign_node.name)
                .map_err(|err| err.with_span(self.current_span))?;
        }

        let right_expr_idx: usize =
            self.gen_named_expr_ir(&assign_node.name, *assign_node.value, &expect_byte, &attr);
        let dst_idx = self.gen_expr_ir(*assign_node.dst, &expect_byte);

        Ok(inst::Inst::AssignVar {
            name: assign_node.name.to_string(),
            dst: dst_idx,
            value: right_expr_idx,
        })
    }

    pub(super) fn init_array_node(
        &mut self,
        init_nodes: Vec<node::Expr>,
        expect_byte: &types::Size,
    ) -> inst::Inst {
        let size = match expect_byte {
            types::Size::Array { size, .. } => size,
            _ => panic!("配列の初期化には配列型が必要です"),
        };
        let mut dsts = Vec::new();
        for node in init_nodes.iter() {
            let dst = self.gen_expr_ir(node.clone(), &size);
            dsts.push(dst);
        }
        inst::Inst::InitArr(dsts)
    }

    #[inline(always)]
    pub(super) fn ref_array_node(
        &mut self,
        dst: node::Expr,
        index: node::Expr,
        name: &str,
        expect_byte: &types::Size,
    ) -> inst::Inst {
        // 配列/範囲付きポインタの添字が範囲を超えていないかを確認する
        self.arr_idx_checker(name, &index);
        self.range_ptr_checker(name, &index);

        inst::Inst::InsertArr {
            name: name.to_string(),
            dst: self.gen_expr_ir(dst, &expect_byte),
            index: self.gen_expr_ir(index, &expect_byte),
        }
    }

    pub(super) fn def_var_node(
        &mut self,
        var: node::DefineVar,
        expect_byte: &types::Size,
        var_attr: &parse::VarMutAttr,
    ) -> Result<inst::Inst, err::ErrKind> {
        // `register_ty`を指定しない = `var.ty`をそのまま登録する
        self.def_var_node_with_register_ty(var, expect_byte, var_attr, None)
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
    fn def_var_node_with_register_ty(
        &mut self,
        mut var: node::DefineVar,
        expect_byte: &types::Size,
        var_attr: &parse::VarMutAttr,
        register_ty: Option<node::TyNode>,
    ) -> Result<inst::Inst, err::ErrKind> {
        match &var.ty.clone() {
            node::TyNode::ConstractMust(constract) | node::TyNode::ConstractOf(constract) => {
                let constract_ty = var.ty.clone();
                let mut inner_var = var.clone();
                inner_var.ty = constract.unwrap_ty();

                let value_idx = self.gen_named_expr_ir(
                    &var.name,
                    *var.value,
                    &self.size_of(&var.ty),
                    &var.var_attr,
                );
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
            node::TyNode::Stack { .. } => {
                // 確保するスタックを増やす
                self.stack_counter(&var.ty);
                self.gen_mem_def_var(var)
            }
            node::TyNode::Static { .. } => self.gen_mem_def_var(var),
            node::TyNode::Ty(ty_name) => {
                let value_idx = self.gen_named_expr_ir(
                    &var.name,
                    *var.value,
                    &self.size_of(&var.ty),
                    &var.var_attr,
                );
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
                // 実際の長さ
                let base_range = self.get_ast_len(&val).unwrap();

                let value_idx =
                    self.gen_named_expr_ir(&var.name, val, &self.size_of(&var.ty), &var_attr);
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
                    size: types::Size::build_ptr_ty(&*ty_name, r),
                    dst: self.id_counter,
                    src: value_idx,
                };
                return Ok(inst);
            }
            t => panic!("{:?}", t),
        }
    }

    pub(super) fn enum_variant_node(
        &mut self,
        name: &str,
        variant: &String,
        expect_byte: &types::Size,
    ) -> inst::Inst {
        let enum_def = self
            .enum_tree
            .get(name)
            .unwrap_or_else(|| panic!("未定義の列挙型です: {}", name));
        let variant_index = enum_def
            .variants
            .iter()
            .position(|v| &v == &variant)
            .unwrap_or_else(|| panic!("列挙型 `{}` にメンバ `{}` は存在しません", name, variant));
        inst::Inst::gen_num(&variant_index.to_string(), &expect_byte, self.id_counter)
    }

    pub(super) fn init_struct_node(
        &mut self,
        name: &String,
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
            .unwrap_or_else(|| panic!("未定義の構造体です: {}", name));

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

            let value_idx = self.gen_expr_ir(*field_expr, &field_size);
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
}
