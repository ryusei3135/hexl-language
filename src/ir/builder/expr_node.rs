use super::*;

impl IR {
    fn get_ast_len(
        &self, 
        ast: &node::Expr
    ) -> Option<usize> {
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
        is_mut: &bool,
    ) -> usize {
        if let node::Expr::Scope { scope, target } = expr {
            self.expr_counter += 1;
            let inst = self.scope_node(
                &scope,
                target,
                Some(var_name), 
                &is_mut
            ).unwrap();
            self.ir_tree.push(inst);
            self.id_counter += 1;
            self.id_counter - 1
        } else {
            self.gen_expr_ir(expr, expect_byte)
        }
    }

    pub fn assign_expr_node(
        &mut self,
        assign_node: node::AssignVar,
        expect_byte: &types::Size,
        is_mut: &bool,
    ) -> Result<inst::Inst, err::ErrKind> {
        // `must`の契約を持つ変数は、`mut`かどうかに関わらず
        // 再代入できない(契約が別の値にすり替わってしまうため)
        if self.var_tree.is_constract_must(&assign_node.name) {
            CompileErr::assign_to_must_var(&assign_node.name)
                .map_err(|err| err.with_span(self.current_span))?;
        }

        if is_mut == &false {
            CompileErr::assign_to_imm_var(&assign_node.name)
                .map_err(|err| err.with_span(self.current_span))?;
        }

        let right_expr_idx: usize =
            self.gen_named_expr_ir(
                &assign_node.name, 
                *assign_node.value, 
                &expect_byte,
                &is_mut 
            );
        let dst_idx = self.gen_expr_ir(
            *assign_node.dst, 
            &expect_byte
        );

        Ok(inst::Inst::AssignVar {
            name: assign_node.name.to_string(),
            dst: dst_idx,
            value: right_expr_idx,
        })
    }

    pub fn init_array_node(
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

    pub fn ref_array_node(
        &mut self,
        dst: node::Expr,
        index: node::Expr,
        name: &String,
        expect_byte: &types::Size,
    ) -> inst::Inst {
        let dst = self.gen_expr_ir(dst, &expect_byte);
        inst::Inst::InsertArr {
            name: name.to_string(),
            dst,
            index: self.gen_expr_ir(index, &expect_byte),
        }
    }

    pub fn def_var_node(
        &mut self,
        mut var: node::DefineVar,
        expect_byte: &types::Size,
        is_mut: &bool,
    ) -> Result<inst::Inst, err::ErrKind> {
        match &var.ty.clone() {
            node::TyNode::ConstractMust(constract)
            | node::TyNode::ConstractOf(constract) => {
                let constract_ty = var.ty.clone();
                let var_name = var.name.clone();

                let mut inner_var = var.clone();
                inner_var.ty = constract.unwrap_ty();

                let inst = self.def_var_node(
                    inner_var, 
                    expect_byte, 
                    is_mut
                );
                self.var_tree.overwrite_ty(&var_name, &constract_ty);
                inst
            }
            node::TyNode::Stack { .. } => {
                // 確保するスタックを増やす
                self.stack_counter(&var.ty);
                self.gen_mem_def_var(var)
            }
            node::TyNode::Static { .. } => self.gen_mem_def_var(var),
            node::TyNode::Ty(ref ty_name) => {
                let value_idx =
                    self.gen_named_expr_ir(
                        &var.name, 
                        *var.value, 
                        &self.size_of(&var.ty),
                        &is_mut
                    );
                let _ = self.var_tree
                    .push::<'l'>(
                        &var.name, 
                        &self.id_counter, 
                        &var.ty,
                        &is_mut,
                    )?;
                let inst = inst::Inst::Mov {
                    name: Some(mem::take(&mut var.name)),
                    size: self.size_of(&node::TyNode::Ty(ty_name.to_string())),
                    dst: self.id_counter,
                    src: value_idx,
                };
                return Ok(inst);
            }
            node::TyNode::Pointer {
                ty_name,
                mut range,
                ..
            } => {
                // `TyNode::Ty`と同じ理由で、`var.name`をそのまま
                // `gen_named_expr_ir`に渡す(詳細は上のコメントを参照)
                let val = *var.value;
                let base_range = self.get_ast_len(&val).unwrap();

                let value_idx =
                    self.gen_named_expr_ir(
                        &var.name, 
                        val, 
                        &self.size_of(&var.ty), 
                        &is_mut
                    );
                // `TyNode::Ty`と同じ理由で、`Mov`自身のindexを登録する
                // (詳細は上の`TyNode::Ty`分岐のコメントを参照)
                let _ = self.var_tree
                    .push::<'l'>(
                        &var.name, 
                        &self.id_counter, 
                        &var.ty, 
                        &is_mut
                    )?;

                if range.is_none() {
                    range = Some((0, base_range));
                } else {
                    // ポインタの範囲指定がある場合、指定された範囲と初期値の長さが一致するか確認する
                    if range.unwrap() != (0, base_range) {
                        CompileErr::ptr_range_len_mismatch(
                            range.unwrap(), 
                            base_range
                        )
                        .map_err(|err| err.with_span(self.current_span))
                        .unwrap();
                    }
                }
                let inst = inst::Inst::Mov {
                    name: Some(mem::take(&mut var.name)),
                    size: types::Size::build_ptr_ty(&*ty_name, range.clone()),
                    dst: self.id_counter,
                    src: value_idx,
                };
                return Ok(inst);
            }
            t => panic!("{:?}", t),
        }
    }

    pub fn enum_variant_node(
        &mut self,
        name: &String,
        variant: &String,
        expect_byte: &types::Size,
    ) -> inst::Inst {
        let enum_def = self
            .enum_tree
            .get(&name.to_string())
            .unwrap_or_else(|| panic!("未定義の列挙型です: {}", name));
        let variant_index = enum_def
            .variants
            .iter()
            .position(|v| &v == &variant)
            .unwrap_or_else(|| panic!("列挙型 `{}` にメンバ `{}` は存在しません", name, variant));
        inst::Inst::gen_num(
            &variant_index.to_string(), 
            &expect_byte, 
            self.id_counter
        )
    }

    pub fn init_struct_node(
        &mut self,
        name: &String,
        fields: &mut HashMap<String, Box<node::Expr>>
    ) -> inst::Inst {
        // 構造体のメゾットを処理中かつ初期化する構造体が`self`
        let struct_name = if self.this_is_self {
            self.var_tree.get_ty_name(name)
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
        inst::Inst::Struct {
            name: name.to_string(),
            mem: mem_insts,
            is_self: self.this_is_self && self.var_tree.is_self_ty(&name),
        }
    }
}
