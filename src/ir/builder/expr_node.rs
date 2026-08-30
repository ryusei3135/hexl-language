use super::*;

impl IR {
    /// `gen_expr_ir`とほぼ同じ処理を行うが、式が`Point.new()`のような
    /// 構造体を返すスコープ呼び出し(`node::Expr::Scope`)だった場合、
    /// この式の結果を格納する予定の変数名(`var_name`)を`scope_node`に
    /// そのまま渡す。これにより、構造体の初期化のために確保する
    /// 一時的なスタック領域が、コンパイラ内部の合成名(`__self_N`)
    /// ではなく、`var_name`(元の変数名)で`var_tree`に登録される
    /// (詳細は`src/ir/builder/scope.rs`の`scope_node`を参照)。
    ///
    /// `Point.new()`以外の式(数値や別の関数呼び出しなど)の場合は、
    /// 通常通り`gen_expr_ir`にそのまま委譲する
    fn gen_named_expr_ir(
        &mut self,
        var_name: &String,
        expr: node::Expr,
        expect_byte: &types::Size,
    ) -> usize {
        if let node::Expr::Scope { scope, target } = expr {
            self.expr_counter += 1;
            let inst = self.scope_node(&scope, target, Some(var_name));
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
    ) -> inst::Inst {
        let right_expr_idx =
            self.gen_named_expr_ir(&assign_node.name, *assign_node.value, &expect_byte);
        let dst_idx = self.gen_expr_ir(*assign_node.dst, &expect_byte);

        inst::Inst::AssignVar {
            name: assign_node.name.to_string(),
            dst: dst_idx,
            value: right_expr_idx,
        }
    }

    pub fn init_array_node(
        &mut self,
        init_nodes: Vec<node::Expr>,
        expect_byte: &types::Size,
    ) -> inst::Inst {
        let types::Size::Array { size, .. } = expect_byte else {
            panic!()
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
    ) -> inst::Inst {
        match &var.ty.clone() {
            node::TyNode::Stack { .. } => {
                // 確保するスタックを増やす
                self.stack_counter(&var.ty);
                self.gen_mem_def_var(var)
            }
            node::TyNode::Static { .. } => self.gen_mem_def_var(var),
            node::TyNode::Ty(ref ty_name) => {
                // `var.value`が`Point.new()`のような構造体を返す
                // スコープ呼び出しの場合、`var.name`(元の変数名)を
                // `gen_named_expr_ir`経由で`scope_node`に渡すことで、
                // `__self_N`のような合成名を作らせないようにする
                let value_idx =
                    self.gen_named_expr_ir(&var.name, *var.value, &self.size_of(&var.ty));
                // 変数の位置として登録するのは、初期化子の式
                // (`value_idx`、例えば`Name::new()`を表す`CallFunc`
                //  ノードそのもの)ではなく、これから生成する`Mov`
                // 自身のindex(`self.id_counter`、下の`dst`と同じ値)
                // でなければならない。
                //
                // `value_idx`をそのまま登録してしまうと、後で
                // `a.add()`のように`self`のアドレスとして変数`a`が
                // 再度参照された際、`GetAddress(Var("a"))`が
                // `value_idx`（＝`CallFunc`ノード自身）を直接指す
                // ことになる。アセンブリ生成時、`GetAddress`は
                // 参照先をそのまま`extract_operand_text`で解決する
                // ため、`CallFunc`ノードの「未評価の関数呼び出し」
                // が再度実行されてしまい、`call new`が二重に
                // 生成されるバグの原因になっていた。
                // `Mov`自身のindexを登録しておけば、再参照時は
                // 既に`var_hash_map`へ登録済みのレジスタ/メモリを
                // 指すようになり、副作用が繰り返されることはない。
                self.var_tree
                    .push::<'l'>(&var.name, &self.id_counter, &var.ty);
                inst::Inst::Mov {
                    name: Some(mem::take(&mut var.name)),
                    size: self.size_of(&node::TyNode::Ty(ty_name.to_string())),
                    dst: self.id_counter,
                    src: value_idx,
                }
            }
            node::TyNode::Pointer { ty_name, .. } => {
                // `TyNode::Ty`と同じ理由で、`var.name`をそのまま
                // `gen_named_expr_ir`に渡す(詳細は上のコメントを参照)
                let value_idx =
                    self.gen_named_expr_ir(&var.name, *var.value, &self.size_of(&var.ty));
                // `TyNode::Ty`と同じ理由で、`Mov`自身のindexを登録する
                // (詳細は上の`TyNode::Ty`分岐のコメントを参照)
                self.var_tree
                    .push::<'l'>(&var.name, &self.id_counter, &var.ty);
                inst::Inst::Mov {
                    name: Some(mem::take(&mut var.name)),
                    size: types::Size::build_ptr_ty(&*ty_name),
                    dst: self.id_counter,
                    src: value_idx,
                }
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
        inst::Inst::gen_num(&variant_index.to_string(), &expect_byte, self.id_counter)
    }

    pub fn init_struct_node(
        &mut self,
        name: &String,
        fields: &mut HashMap<String, Box<node::Expr>>,
        expect_byte: &types::Size,
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
            .clone()
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
