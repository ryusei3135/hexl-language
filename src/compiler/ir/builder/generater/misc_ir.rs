//! その他: `Self`型の解決、メゾットの展開、エラーの報告、ラベル発番、テスト用の取得関数

use super::*;

impl IR {
    /// テスト用のヘルパー。
    /// `Self`をメゾットの所属構造体名に解決し、最初の引数が`self`の場合は
    /// そのまま構造体へのポインタ型に変換する。
    pub(in crate::compiler::ir) fn resolved_self_ty(
        &self,
        mut func: ast::FuncDefine,
    ) -> ast::FuncDefine {
        let first_param_is_self = func
            .params
            .first()
            .is_some_and(|param| Self::ty_contains_self(&param.ty));

        if let Some(first) = func.params.first_mut() {
            if Self::ty_contains_self(&first.ty) {
                first.ty = Self::resolve_self_like_ty(&first.ty);
            }
        }

        func.params
            .iter()
            .enumerate()
            .skip(1)
            .for_each(|(index, param)| {
                if Self::ty_contains_self(&param.ty) {
                    panic!(
                        "Self型は第一引数以外では使えません: param#{index} {:?}",
                        param.ty
                    );
                }
            });

        if first_param_is_self {
            func.ret_ty = Self::resolve_self_like_ty(&func.ret_ty);
        } else if let ast::TyNode::SelfTy(name) = &func.ret_ty {
            // `new(): Self`のように、第一引数が`Self`ではないメゾットの`Self`の戻り値は、
            // 自身の構造体を値で返す(`ret Self { .. }`)。
            // 構造体を返す通常の関数(`f(): Name`)と同じ扱いにする
            func.ret_ty = ast::TyNode::Ty(name.clone());
        } else if Self::ty_contains_self(&func.ret_ty) {
            panic!(
                "Self型の戻り値は、第一引数がSelfのメソッドでのみ使えます: {:?}",
                func.ret_ty
            );
        }

        if let Some(owner) = func.module.as_deref() {
            if let Some(contract) = func.contract.take() {
                func.contract = Some(Self::resolve_method_contract(
                    &contract,
                    owner,
                    &func.ret_ty,
                ));
            }
        }

        func
    }

    /// メソッドの契約終了先を、所属する型のメソッド名として解決する
    fn resolve_method_contract(
        contract: &ast::TyNode,
        owner: &str,
        ret_ty: &ast::TyNode,
    ) -> ast::TyNode {
        match contract {
            ast::TyNode::ConstractMust(inner) => {
                let name = inner.must_name().map(|name| format!("{owner}::{name}"));
                ast::ConstractTy::new::<{ ast::IS_MUST }>(name, None, Box::new(ret_ty.clone()))
            }
            ast::TyNode::ConstractOf(inner) => {
                let name = inner.of_name().map(|name| format!("{owner}::{name}"));
                ast::ConstractTy::new::<{ ast::IS_OF }>(None, name, Box::new(ret_ty.clone()))
            }
            other => other.clone(),
        }
    }

    #[inline(always)]
    pub(in crate::compiler::ir) fn resolve_self_ty(
        &self,
        func: &ast::FuncDefine,
    ) -> ast::FuncDefine {
        self.resolved_self_ty(func.to_owned())
    }

    pub(in crate::compiler::ir) fn ty_contains_self(ty: &ast::TyNode) -> bool {
        match ty {
            ast::TyNode::SelfTy(..) => true,
            ast::TyNode::Pointer { ty_name, .. } => Self::ty_contains_self(ty_name),
            ast::TyNode::RefTy(inner) | ast::TyNode::RefContract(inner) => {
                Self::ty_contains_self(inner)
            }
            _ => false,
        }
    }

    pub(in crate::compiler::ir) fn resolve_self_like_ty(ty: &ast::TyNode) -> ast::TyNode {
        match ty {
            ast::TyNode::SelfTy(name) => ast::TyNode::Pointer {
                is_const: false,
                ty_name: Box::new(ast::TyNode::Ty(name.clone())),
                range: None,
            },
            ast::TyNode::Pointer {
                is_const,
                ty_name,
                range,
            } => {
                if Self::ty_contains_self(ty_name) {
                    ast::TyNode::Pointer {
                        is_const: *is_const,
                        ty_name: Box::new(match &**ty_name {
                            ast::TyNode::SelfTy(name) => ast::TyNode::Ty(name.clone()),
                            other => other.clone(),
                        }),
                        range: range.clone(),
                    }
                } else {
                    ty.clone()
                }
            }
            ast::TyNode::RefTy(inner) => {
                if Self::ty_contains_self(inner) {
                    ast::TyNode::RefTy(Box::new(Self::resolve_self_like_ty(inner)))
                } else {
                    ty.clone()
                }
            }
            ast::TyNode::RefContract(inner) => {
                if Self::ty_contains_self(inner) {
                    ast::TyNode::RefContract(Box::new(Self::resolve_self_like_ty(inner)))
                } else {
                    ty.clone()
                }
            }
            ast::TyNode::ConstractMust(inner) => ast::TyNode::ConstractMust(
                inner.with_ty(Self::resolve_self_like_ty(&inner.unwrap_ty())),
            ),
            ast::TyNode::ConstractOf(inner) => ast::TyNode::ConstractOf(
                inner.with_ty(Self::resolve_self_like_ty(&inner.unwrap_ty())),
            ),
            _ => ty.clone(),
        }
    }

    /// 構造体/バリアント型のメゾットのシグネチャを、関数ツリーに先に登録する
    ///
    /// `expand_methods`は、型が定義された順番にメゾットを展開する。
    /// そのため、型の定義より前の関数から呼び出されたり、
    /// ジェネリクスから作られた型(定義が最後に追加される)のメゾットを
    /// 呼び出したりすると、まだ登録されていないことになる。
    /// 関数と同じように、本体を生成する前にシグネチャだけ登録しておく。
    /// `expand_methods`と同じ`Self`の解決とモジュール名の付け方にする
    pub(in crate::compiler::ir) fn declare_methods(
        &mut self,
        owner: &str,
        methods: &[ast::Group1Node],
    ) {
        for method in methods {
            let ast::Group1Node::FuncDefine(method_info) = method else {
                // 関数定義以外は、`expand_methods`が報告する
                continue;
            };
            let mut method_info = method_info.clone();
            method_info.module = Some(owner.to_string());
            let method_info = self.resolve_self_ty(&method_info);
            self.func_tree.declare(&method_info);
        }
    }

    /// 構造体の中に定義されているメゾット(`StructDefine::methods`)を、
    /// 通常のトップレベルの関数として展開してIRへ変換する
    ///
    /// - メゾットは、そのメゾットが属する構造体の名前をモジュール名として
    ///   持つ関数として登録される(`ast::FuncDefine::self_module_name`)。
    ///   これにより`StructName::method_name(..)`という、通常の
    ///   モジュール(`#include`)経由の関数呼び出しと同じ構文
    ///   (`Expr::Scope`)で呼び出せるようになる
    /// - 第一引数が`Self`型のメゾット(`self: Self`)は`resolve_self_ty`に
    ///   よって、自身の構造体を指すポインタを受け取る関数として解決される
    /// - それ以外の変換処理は、トップレベルの関数定義
    ///   (`Group1Node::FuncDefine`)と全く同じ流れで行う
    pub(in crate::compiler::ir) fn expand_struct_methods<M: ResultMod>(
        &mut self,
        struct_def: &ast::StructDefine,
    ) -> Result<(), err::ErrKind> {
        self.expand_methods::<M>("構造体", &struct_def.name, &struct_def.methods)
    }

    /// バリアント型の中に定義されているメゾット(`VariantDefine::methods`)を、
    /// 通常のトップレベルの関数として展開してIRへ変換する
    /// (処理の流れは`expand_struct_methods`と同じ)
    pub(in crate::compiler::ir) fn expand_variant_methods<M: ResultMod>(
        &mut self,
        variant_def: &ast::VariantDefine,
    ) -> Result<(), err::ErrKind> {
        self.expand_methods::<M>("バリアント型", &variant_def.name, &variant_def.methods)
    }

    /// `expand_struct_methods`/`expand_variant_methods`の共通処理
    /// - `kind`: エラーメッセージ用の、型の種類の名前
    /// - `owner`: メゾットが属する型の名前(モジュール名になる)
    pub(in crate::compiler::ir) fn expand_methods<M: ResultMod>(
        &mut self,
        kind: &str,
        owner: &str,
        methods: &[ast::Group1Node],
    ) -> Result<(), err::ErrKind> {
        self.this_is_self = true;
        // 1つのメゾットでエラーになっても、残りのメゾットは展開を続ける。
        // 途中で打ち切ると、`declare_methods`が先に登録した本体が空のスタブが
        // そのまま残り、アセンブリがラベルとスタックフレームだけになる。
        // 最初のエラーだけを返す
        let mut first_err: Option<err::ErrKind> = None;
        for method in methods {
            let ast::Group1Node::FuncDefine(method_info) = method else {
                panic!(
                    "{}`{}`のメゾットに、関数定義以外のノードが渡されました: {:?}",
                    kind, owner, method
                );
            };
            if method_info.is_declaration_only() {
                continue;
            }
            self.active_struct = Some(owner.to_string());
            let mut method_info = method_info.clone();
            method_info.module = Some(owner.to_string());
            let method_info = self.resolve_self_ty(&method_info);

            if let Err(e) = self.expand_one_method::<M>(&method_info) {
                // TODO(調査用): どのメゾットがなぜ展開に失敗したかを表示する
                eprintln!(
                    "[expand_methods] {}::{} failed: {:?}",
                    owner, method_info.name, e
                );
                // 失敗したメゾットの途中のIRを、次のメゾットへ持ち越さない
                self.ir_tree = Vec::new();
                self.id_counter = 0;
                self.stk_counter = 0;
                first_err.get_or_insert(e);
            }
        }
        let result = match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        };
        self.active_struct = None;
        self.this_is_self = false;
        result
    }

    /// メゾット1つぶんをIRへ展開して`func_tree`へ登録する
    /// (`init_def_fn_info`と同じ流れ。使用回数の数え直しと`:=`の型推論も行う)
    fn expand_one_method<M: ResultMod>(
        &mut self,
        method_info: &ast::FuncDefine,
    ) -> Result<(), err::ErrKind> {
        // 関数ごとに使用回数を数え直す。
        // (数え直さないと、前のメゾットの回数が混ざったり、引数の回数が
        //  初期化されなかったりして、レジスタの割り当てと「未使用変数の
        //  削除」(`asm_gen/reg_mnger.rs`)が壊れる)
        self.var_counter = counter_mnger::VarCounter::with_arg_count(method_info.params.len());
        // 関数の情報を登録
        self.entry_fn_info(method_info)?;
        self.var_tree = def_tree::VarTree::new();
        let _ = self.push_param_meta_data(&method_info.params)?;
        self.scope_states.clear();
        // `:=`のうち、使われ方から型が決まるものを先に決める
        let mut body = method_info.body.clone();
        infer_auto::resolve_auto_by_usage(&mut body, &method_info.params, &method_info.ret_ty);
        self.gen_inst::<M>(&body)?;
        // 関数の処理内容をpush
        self.push_fn_ir_tree(method_info);
        // 使うデータを初期化
        self.ir_tree = Vec::new();
        self.id_counter = 0;
        Ok(())
    }

    /// ユーザーのソースコード側の誤りに起因しうる`Result`を、
    /// 生の`.unwrap()`で握りつぶす代わりに使うヘルパー。
    /// `Err`の場合はコンパイラを異常終了させる代わりに、
    /// エラー内容を表示してから終了する
    /// (`gen_expr_ir`など戻り値が`usize`の関数の中では
    /// `?`が使えないため、この形にしている)
    pub(in crate::compiler::ir) fn unwrap_or_report<T>(
        &self,
        result: Result<T, err::ErrKind>,
    ) -> T {
        match result {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: {:?}", e);
                // テスト中は`exit`するとテストのプロセス全体が終了してしまうので、
                // panicして`catch_unwind`で捕まえられるようにする
                if cfg!(test) {
                    panic!("{:?}", e);
                }
                std::process::exit(1);
            }
        }
    }

    /// `ir/insert/*`からも使う
    pub(in crate::compiler::ir) fn next_pattern_label(&mut self) -> usize {
        loop {
            let label = self.pattern_labels;
            self.pattern_labels += 1;
            let label_text = format!("L{}", label);
            if !self.ir_tree.iter().any(|inst| match inst {
                inst::Inst::Block(existing)
                | inst::Inst::Jmp(existing)
                | inst::Inst::ExpectJmp(existing) => existing == &label_text,
                _ => false,
            }) {
                return label;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn test_only_get_func_body(&self, name: &str) -> &[inst::Inst] {
        &self
            .func_tree
            .func
            .get(name)
            .expect(format!("name -> {}", name).as_str())
            .body
    }

    /// 構造体のメゾットとして展開された関数の処理内容を取得する
    /// (テスト用)
    #[cfg(test)]
    pub(crate) fn test_only_get_method_body(&self, module: &str, name: &str) -> &[inst::Inst] {
        &self
            .func_tree
            .func
            .get(&format!("{}::{}", module, name))
            .unwrap()
            .body
    }
}

#[cfg(test)]
mod self_ty_tests {
    use super::*;
    use crate::compiler::ArgsNode;

    fn make_func(name: &str, params: Vec<ArgsNode>, ret_ty: ast::TyNode) -> ast::FuncDefine {
        let mut func = match ast::FuncDefine::new(
            name.to_string(),
            params,
            ret_ty,
            false,
            test_span(),
            None,
        ) {
            Group1Node::FuncDefine(func) => func,
            _ => unreachable!(),
        };
        func.module = Some("Name".to_string());
        func
    }

    #[test]
    fn first_arg_self_becomes_pointer_to_struct() {
        // `func(self: Self, param: int): int` の第一引数は
        // 構造体`Name`へのポインタとして解決される
        let ir = new_test_ir();
        let func = make_func(
            "func",
            vec![
                ArgsNode {
                    name: "self".to_string(),
                    ty: ast::TyNode::SelfTy("Name".to_string()),
                    unsafe_contract_ptr_arithmetic: false,
                    var_attr: parse::VarMutAttr::Invar,
                    span: test_span(),
                },
                ArgsNode {
                    name: "param".to_string(),
                    ty: ast::TyNode::Ty("int".to_string()),
                    unsafe_contract_ptr_arithmetic: false,
                    var_attr: parse::VarMutAttr::Invar,
                    span: test_span(),
                },
            ],
            ast::TyNode::Ty("int".to_string()),
        );

        let resolved = ir.resolve_self_ty(&func);

        assert_eq!(
            resolved.params[0].ty,
            ast::TyNode::Pointer {
                is_const: false,
                ty_name: Box::new(ast::TyNode::Ty("Name".to_string())),
                range: None,
            }
        );
        // 第一引数以外はそのまま
        assert_eq!(resolved.params[1].ty, ast::TyNode::Ty("int".to_string()));
    }

    #[test]
    #[should_panic]
    fn self_in_non_first_arg_panics() {
        // 第一引数以外に`Self`が使われている場合はエラー(panic)にする
        let ir = new_test_ir();
        let func = make_func(
            "invalid",
            vec![
                ArgsNode {
                    name: "param".to_string(),
                    ty: ast::TyNode::Ty("int".to_string()),
                    unsafe_contract_ptr_arithmetic: false,
                    var_attr: parse::VarMutAttr::Invar,
                    span: test_span(),
                },
                ArgsNode {
                    name: "self".to_string(),
                    ty: ast::TyNode::SelfTy("Name".to_string()),
                    unsafe_contract_ptr_arithmetic: false,
                    var_attr: parse::VarMutAttr::Invar,
                    span: test_span(),
                },
            ],
            ast::TyNode::Ty("int".to_string()),
        );

        let _ = ir.resolve_self_ty(&func);
    }

    #[test]
    fn self_arg_and_self_return_resolves_to_matching_pointer() {
        // `func2(self: Self): Self` のように、第一引数がSelfかつ
        // 戻り値もSelfの場合、戻り値も`self`と同じポインタ型になる
        let ir = new_test_ir();
        let func = make_func(
            "func2",
            vec![ArgsNode {
                name: "self".to_string(),
                ty: ast::TyNode::SelfTy("Name".to_string()),
                unsafe_contract_ptr_arithmetic: false,
                var_attr: parse::VarMutAttr::Invar,
                span: test_span(),
            }],
            ast::TyNode::SelfTy("Name".to_string()),
        );

        let resolved = ir.resolve_self_ty(&func);

        let expected_ptr = ast::TyNode::Pointer {
            is_const: false,
            ty_name: Box::new(ast::TyNode::Ty("Name".to_string())),
            range: None,
        };
        assert_eq!(resolved.params[0].ty, expected_ptr.clone());
        assert_eq!(resolved.ret_ty, expected_ptr);
    }
}

#[cfg(test)]
mod struct_method_expand_tests {
    use super::*;

    #[test]
    fn method_is_registered_as_function_with_struct_as_module() {
        // 構造体`Point`のメゾット`get_num`が、モジュール名`Point`を
        // 持つ通常の関数としてIRに展開されることを確認する
        let nodes =
            parse_test_source("struct Point { x: int pub get_num(self: Self): int { ret 1 } }");

        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();

        // `Point::get_num`として展開され、呼び出せる状態になっている
        let body = ir.test_only_get_method_body("Point", "get_num");
        assert!(
            body.iter().any(|inst| matches!(inst, inst::Inst::Ret(_))),
            "メゾットの中身がそのままIRに変換されている必要がある: {:?}",
            body
        );

        // メゾットの第一引数(`self`)は、構造体`Point`へのポインタとして
        // 解決されている必要がある
        let registered = ir
            .func_tree
            .get(&"get_num".to_string(), Some(&"Point".to_string()))
            .unwrap();
        assert_eq!(
            registered.args[0].ty,
            ast::TyNode::Pointer {
                is_const: false,
                ty_name: Box::new(ast::TyNode::Ty("Point".to_string())),
                range: None,
            }
        );

        // モジュール名(構造体名)を指定しない場合は解決できない
        // (トップレベルの関数と名前空間が混ざらないようにするため)
        assert!(ir.func_tree.get(&"get_num".to_string(), None).is_none());
    }

    #[test]
    fn declaration_only_method_is_not_generated() {
        let mut nodes =
            parse_test_source("struct Point { x: int pub get(self: Self): int { ret 1 } }");
        let ast::Group1Node::StructDefine(def) = &mut nodes[0] else {
            panic!("expected struct definition");
        };
        let ast::Group1Node::FuncDefine(method) = &mut def.methods[0] else {
            panic!("expected method definition");
        };
        *method = method.declaration_only();

        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();

        assert!(ir.test_only_get_method_body("Point", "get").is_empty());
    }

    #[test]
    fn method_can_be_called_before_struct_definition() {
        // 型の定義より前に書かれた関数からも、メゾットを呼び出せる
        let nodes = parse_test_source(
            "main(): int { ret Point::answer() } struct Point { pub answer(): int { ret 42 } }",
        );

        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();

        let body = ir.test_only_get_func_body("main");
        assert!(
            body.iter()
                .any(|inst| matches!(inst, inst::Inst::CallFunc(_)))
        );
    }

    #[test]
    fn generic_struct_method_can_be_called() {
        // ジェネリクスから作った型(`Pair$int`)は、使った関数より後ろに
        // 追加されるが、そのメゾットも呼び出せる
        let nodes = parse_test_source(
            "struct Pair<T> { a: T pub new(): Self { ret Self { a: 0 } } } main(): int { p: Pair<int> = Pair<int>::new() ret 0 }",
        );

        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();

        let body = ir.test_only_get_func_body("main");
        assert!(
            body.iter()
                .any(|inst| matches!(inst, inst::Inst::CallFunc(_)))
        );
    }

    #[test]
    fn method_can_be_called_via_struct_name_scope() {
        // `self`を取らない、構造体の「静的メゾット」を定義し、
        // `Point::answer()`のように呼び出せることを確認する
        let nodes = parse_test_source(
            "struct Point { pub answer(): int { ret 42 } } main(): int { ret Point::answer() }",
        );

        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();

        let body = ir.test_only_get_func_body("main");
        assert!(
            body.iter()
                .any(|inst| matches!(inst, inst::Inst::CallFunc(_))),
            "`Point::answer()`の呼び出しが生成される必要がある: {:?}",
            body
        );
    }

    #[test]
    fn method_return_contract_targets_a_method_on_its_struct() {
        let nodes = parse_test_source(
            "struct Point { \
                 x: int \
                 pub new(): Self must=del { ret Self { x: 1 } } \
                 pub get(self: Self*): int { ret 1 } \
                 pub del(self: Self* of new) {} \
             } \
                 _start(): int { point: Point = Point::new() point.get() point.del() ret 0 }",
        );

        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();

        let make = ir.func_tree.get("new", Some("Point")).unwrap();
        assert_eq!(
            make.ret_ty,
            Some(ast::ConstractTy::new::<{ ast::IS_MUST }>(
                Some("Point::del".to_string()),
                None,
                Box::new(ast::TyNode::Ty("Point".to_string())),
            ))
        );

        assert!(ir.var_tree.get_ty_node("point").unwrap().is_contract_must());
    }

    #[test]
    fn struct_field_consumes_its_initializer_return_contract() {
        let nodes = parse_test_source(
            "struct Holder { \
                 value: byte*\"unlimit\" must=free \
                 count: u64 \
             } \
             alloc(): byte* must=free { ret 0 } \
             main(): int { \
                 holder: Holder = Holder { value: alloc(), count: 0 } \
                 ret 0 \
             }",
        );

        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();
    }
}

#[cfg(test)]
mod method_call_via_member_tests {
    use super::*;

    #[test]
    fn calling_method_via_dot_syntax_passes_implicit_self_pointer() {
        // `p.get_num()`のように、変数のメンバーアクセス経由で
        // メゾットを呼び出すと、暗黙的に`p`のアドレスが
        // 第一引数(`self`)として渡されることを確認する
        let nodes = parse_test_source(
            "struct Point { x: int pub get_num(self: Self): int { ret 7 } } main(): int { p: Point = Point { x: 1 } ret p.get_num() }",
        );

        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();

        let body = ir.test_only_get_func_body("main");

        // `self`のアドレスを取得する`GetAddress`命令が生成されている
        assert!(
            body.iter()
                .any(|inst| matches!(inst, inst::Inst::GetAddress(_))),
            "`p.get_num()`は暗黙的に`p`のアドレスを渡す必要がある: {:?}",
            body
        );
        // 関数呼び出し命令(`get_num`)が生成されている
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::CallFunc(inst::CallFuncMetaData{name, ..}) if name == "get_num"
            )),
            "{:?}",
            body
        );
    }

    #[test]
    fn calling_method_on_struct_field_passes_field_address_as_self() {
        let nodes = parse_test_source(
            "struct Inner { \
                 pub get_num(self: Self*): int { ret 7 } \
             } \
             struct Outer { \
                 inner: Inner \
                 pub get_inner_num(self: Self*): int { ret self->inner.get_num() } \
             } \
             main(): int { ret 0 }",
        );

        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();

        let body = ir.test_only_get_method_body("Outer", "get_inner_num");
        assert!(
            body.iter()
                .any(|inst| matches!(inst, inst::Inst::GetAddress(_))),
            "the field address must be passed as the implicit self argument: {:?}",
            body
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::CallFunc(inst::CallFuncMetaData { name, .. }) if name == "get_num"
            )),
            "{:?}",
            body
        );
    }
}
