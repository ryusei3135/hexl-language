//! データの処理: 変数定義、配列、構造体、バリアント型(列挙型)

use super::*;

impl IR {
    fn get_ast_len(&self, ast: &node::Expr) -> Option<usize> {
        match &ast.node {
            node::ExprNode::Str(value) => Some(value.len()),
            node::ExprNode::Array(values) => Some(values.len()),
            _ => None,
        }
    }

    pub(in super::super) fn init_array_node<M: ResultMod>(
        &mut self,
        init_nodes: Vec<node::Expr>,
        expect_byte: &types::Size,
    ) -> Result<inst::Inst, err::ErrKind> {
        let size = match expect_byte {
            types::Size::Array { size, .. } => size,
            _ => panic!("配列の初期化には配列型が必要です"),
        };
        let mut dsts = Vec::new();
        for n in init_nodes.iter() {
            let dst = self.gen_expr_ir::<M>(&n, Some(&size))?;
            dsts.push(dst);
        }
        Ok(inst::Inst::InitArr(dsts))
    }

    #[inline(always)]
    pub(in super::super) fn ref_array_node<M: ResultMod>(
        &mut self,
        dst: &node::Expr,
        index: &node::Expr,
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

    pub(in super::super) fn def_var_node<M: ResultMod>(
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
                    let node::Expr {
                        node: value_node,
                        span,
                    } = *var.value;
                    let init_values = match value_node {
                        node::ExprNode::Array(values) => values,
                        value_node => vec![node::Expr::new(value_node, std::rc::Rc::clone(&span))],
                    };
                    let array = node::Expr::new(node::ExprNode::Array(init_values), span);
                    let value_idx = self.gen_expr_ir::<M>(&array, Some(&array_ty))?;
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
    pub(in super::super) fn enum_variant_node(
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

    pub(in super::super) fn init_struct_node<M: ResultMod>(
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

            let value_idx = self.gen_expr_ir::<M>(field_expr, Some(&field_size))?;
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
    pub(in super::super) fn init_variant_node<M: ResultMod>(
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
                let value_idx = self.gen_expr_ir::<M>(field_expr, Some(&field_size))?;
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
