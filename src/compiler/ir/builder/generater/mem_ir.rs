//! メモリ(スタック/静的領域)の確保と、メモリ上の変数定義のIR生成

use super::*;

impl IR {
    pub(in crate::compiler::ir) fn stack_counter(&mut self, size: &ast::TyNode) {
        let result = self.try_size_or_emit_struct(size);
        let ty = self.unwrap_or_report(result);
        self.stk_counter += ty.to_bytes();
    }

    /// スタック/静的領域に確保する変数の定義から
    /// `inst::Inst::Struct(Vec<inst::MemoryInst>)` を生成する
    ///
    /// - `name: [ty] = 100` のように長さの指定が無い場合は要素数1
    /// - `name: [ty 4] = {100, 100, 100, 100}` のように配列リテラルが
    ///   与えられた場合は、要素ごとに値を生成する
    pub(in crate::compiler::ir) fn gen_mem_def_var<M: ResultMod>(
        &mut self,
        mut var: ast::DefineVar,
    ) -> Result<inst::Inst, err::ErrKind> {
        let (ref ty_name, len, is_static) = match &var.ty {
            ast::TyNode::Stack { name, len } => (name.clone(), *len, false),
            ast::TyNode::Static { name, len } => (name.clone(), *len, true),
            _ => panic!("gen_mem_def_var: スタック/静的領域の型ではありません"),
        };
        let elem_size = self.size_of(&ty_name);

        // 配列リテラルなら要素ごとに展開し、単一の値ならそのまま1要素として扱う
        let values = match (*var.value).node {
            ast::ExprNode::Array(items) => items,
            _ => vec![(*var.value).clone()],
        };

        let mut value_idx = Vec::new();
        for value in values {
            value_idx.push(M::handle(self.gen_expr_ir::<M>(&value, Some(&elem_size)))?);
        }

        // メモリ領域の情報を作成
        // 先頭にどの領域(スタック/静的)かを示し、続けて要素分のバイト数を積む
        let mem_kind = if is_static {
            inst::MemoryKind::Static
        } else {
            inst::MemoryKind::Stack
        };
        let mem_insts = inst::MemoryInst::Memory {
            name: var.name.to_string(),
            size: elem_size.clone(),
            src: value_idx,
            kind: mem_kind,
            dst: self.ir_tree.len(),
        };
        for _ in 0..len {
            //mem_insts.push(inst::MemoryInst::Byte(elem_size.to_bytes()));
        }

        // 変数はスタック/静的領域に配置された値を指す必要があるため、
        // 生成した値そのもの(last_value_idx)ではなく、
        // この`MemoryValue`命令自身のid(=mem_insts.dst)を
        // 変数の指す先として登録する。
        // (以前はlen == 1の場合に値のidをそのまま使っていたため、
        //  変数を参照した際に即値やレジスタが直接使われてしまい、
        //  スタック/静的領域への書き込みが無視されるバグがあった)
        let var_idx = self.id_counter;
        let _ = self.var_tree.push::<'l'>(
            &mem::take(&mut var.name),
            var_idx,
            &var.ty,
            &var.var_attr,
            Rc::clone(&self.current_span),
            || self.constract_flag.put_var_def(&var.ty),
        )?;

        Ok(inst::Inst::MemoryValue(mem_insts))
    }
}

#[cfg(test)]
mod mem_var_tests {
    use super::*;

    fn build_func_body(src: &str) -> Vec<inst::Inst> {
        let nodes = parse_test_source(src);
        let mut ir = IR::new();
        ir.builder(&nodes).unwrap();
        ir.test_only_get_func_body("main").to_vec()
    }

    #[test]
    fn check_stack_struct_inst() {
        let body = build_func_body("main(): int { a: [int] = 100 }");
        let name = String::from("a");
        assert!(
            body.iter().any(|inst| {
                if let inst::Inst::MemoryValue(inst::MemoryInst::Memory {
                    name: mem_name,
                    size,
                    src,
                    kind,
                    dst,
                }) = inst
                {
                    mem_name == &name
                        && *size == types::Size::DD
                        && src == &vec![0]
                        && *kind == inst::MemoryKind::Stack
                        && *dst == 1
                } else {
                    false
                }
            }),
            "{:?}",
            body
        );
    }

    /*#[test]
    fn check_stack_array_struct_inst() {
        let body = build_func_body("main(): int { a: [int 4] = [100, 100, 100, 100] }");
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Struct(v) if v == &vec![
                    inst::MemoryInst::Stack,
                    inst::MemoryInst::Byte(4),
                    inst::MemoryInst::Byte(4),
                    inst::MemoryInst::Byte(4),
                    inst::MemoryInst::Byte(4),
                ]
            )),
            "{:?}", body
        );
    }*/

    #[test]
    fn check_static_struct_inst() {
        let body = build_func_body("main(): int { a: \"\"[int] = 100 }");
        let name = String::from("a");
        assert!(
            body.iter().any(|inst| {
                if let inst::Inst::MemoryValue(inst::MemoryInst::Memory {
                    name: mem_name,
                    size,
                    src,
                    kind,
                    dst,
                }) = inst
                {
                    mem_name == &name
                        && *size == types::Size::DD
                        && src == &vec![0]
                        && *kind == inst::MemoryKind::Static
                        && *dst == 1
                } else {
                    false
                }
            }),
            "{:?}",
            body
        );
    }

    #[test]
    fn check_variant_enum_like_as_ty_and_access() {
        // 型なしメンバーだけの`variant`(旧列挙型)を型として使い、
        // `Name::Mem`でメンバにアクセスできる
        let body = build_func_body(
            "variant Color { Red Green Blue } main(): int { a: Color = Color::Green }",
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Num { value, .. } if value == "1"
            )),
            "Color::Green は2番目(index 1)のメンバとして展開される必要がある: {:?}",
            body
        );
    }

    #[test]
    fn check_variant_enum_like_forward_reference() {
        // `variant`が使われる場所より後に定義されていても解決できる
        let body = build_func_body(
            "main(): int { a: Color = Color::Blue } variant Color { Red Green Blue }",
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Num { value, .. } if value == "2"
            )),
            "{:?}",
            body
        );
    }

    #[test]
    fn check_combined_struct_variant_program() {
        // 構造体と`variant`を同じプログラム内で型として使えることを確認する
        let body = build_func_body(
            "variant Color { Red Green Blue } struct Point { x: int y: int } main(): int { c: Color = Color::Green p: Point = Point { x: 1 y: 2 } }",
        );
        assert!(
            body.iter()
                .any(|i| matches!(i, inst::Inst::Num{value, ..} if value == "1"))
        );
        assert!(body.iter().any(|i| matches!(
            i,
            inst::Inst::Struct { mem, .. } if mem.len() == 2
        )));
    }

    #[test]
    fn check_variant_with_data_init() {
        // データ付きメンバーの初期化は、タグ付きの`Inst::Variant`になる
        let body = build_func_body("variant V { A B(int) } main(): int { v: V = V { B: 5 } }");
        assert!(
            body.iter().any(|i| matches!(
                i,
                inst::Inst::Variant {
                    tag: 1,
                    value: Some(_),
                    tagged: true,
                    ..
                }
            )),
            "{:?}",
            body
        );
    }

    #[test]
    fn check_variant_typeless_member_in_data_variant() {
        // データ付きメンバーを持つ`variant`の型なしメンバーは`Inst::Variant`(値なし)
        let body = build_func_body("variant V { A B(int) } main(): int { v: V = V::A }");
        assert!(
            body.iter().any(|i| matches!(
                i,
                inst::Inst::Variant {
                    tag: 0,
                    value: None,
                    tagged: true,
                    ..
                }
            )),
            "{:?}",
            body
        );
    }

    #[test]
    fn check_generic_variant_infers_type_from_context() {
        let body = build_func_body(
            "variant Option<T> { Some(T) None } create(): Option<int> { ret Option::None } main(): int { value: int = 10 some: Option<int> = Option::Some(value) none: Option<int> = Option::None }",
        );
        let variants = body
            .iter()
            .filter_map(|inst| match inst {
                inst::Inst::Variant { name, tag, .. } => Some((name.as_str(), tag)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(variants.contains(&("Option$int", 0)), "{:?}", body);
        assert!(variants.contains(&("Option$int", 1)), "{:?}", body);
    }

    #[test]
    fn check_variant_unsafe_has_no_tag() {
        // `"unsafe"`はタグを持たない
        let body = build_func_body(
            "variant \"unsafe\" V { a: int b: i64 } main(): int { v: V = V { b: 5 } }",
        );
        assert!(
            body.iter().any(|i| matches!(
                i,
                inst::Inst::Variant {
                    tagged: false,
                    value: Some(_),
                    ..
                }
            )),
            "{:?}",
            body
        );
    }

    #[test]
    fn check_variant_size() {
        // タグ(4バイト) + 最大のメンバー(i64 = 8バイト)
        let src = "variant V { A B(int) C(i64) } main(): int { v: V = V { C: 1 } }";
        let body = build_func_body(src);
        let size = body.iter().find_map(|i| match i {
            inst::Inst::Variant { size, .. } => Some(size.clone()),
            _ => None,
        });
        assert_eq!(size.map(|s| s.to_bytes()), Some(12), "{:?}", body);
    }

    #[test]
    fn check_struct_as_ty_and_init() {
        // 構造体を型として使い、フィールドを初期化できる
        let body = build_func_body(
            "struct Point { x: int y: int } main(): int { p: Point = Point { x: 1 y: 2 } }",
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Struct { mem: members, .. } if members.len() == 2
                    && members.iter().any(|m| matches!(
                        m,
                        inst::MemoryInst::Member { parent, .. } if parent == "x"
                    ))
                    && members.iter().any(|m| matches!(
                        m,
                        inst::MemoryInst::Member { parent, .. } if parent == "y"
                    ))
            )),
            "{:?}",
            body
        );
    }
}
