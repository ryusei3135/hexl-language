//! その他: `Self`型の解決、メゾットの展開、エラーの報告、ラベル発番、テスト用の取得関数

use super::*;
use crate::compiler::ast::{ArgsNode, Expr, Group1Node};

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

        for (index, param) in func.params.iter().enumerate().skip(1) {
            if Self::ty_contains_self(&param.ty) {
                panic!(
                    "Self型は第一引数以外では使えません: param#{index} {:?}",
                    param.ty
                );
            }
        }

        if first_param_is_self {
            func.ret_ty = Self::resolve_self_like_ty(&func.ret_ty);
        } else if Self::ty_contains_self(&func.ret_ty) {
            panic!(
                "Self型の戻り値は、第一引数がSelfのメソッドでのみ使えます: {:?}",
                func.ret_ty
            );
        }

        func
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
            ast::TyNode::RefTy(inner) => Self::ty_contains_self(inner),
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
            _ => ty.clone(),
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
        for method in methods {
            let ast::Group1Node::FuncDefine(method_info) = method else {
                panic!(
                    "{}`{}`のメゾットに、関数定義以外のノードが渡されました: {:?}",
                    kind, owner, method
                );
            };

            let mut method_info = self.resolve_self_ty(&method_info);
            method_info.module = Some(owner.to_string());

            // 関数の情報を登録
            self.entry_fn_info(&method_info)?;

            self.var_tree = def_tree::VarTree::new();

            let _ = self.push_param_meta_data(&method_info.params)?;
            self.scope_states.clear();
            self.gen_inst::<M>(&method_info.body)?;

            // 関数の処理内容をpush
            self.push_fn_ir_tree(&method_info);
            // 使うデータを初期化
            self.ir_tree = Vec::new();
            self.id_counter = 0;
        }
        self.this_is_self = false;
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

    fn make_func(name: &str, params: Vec<ArgsNode>, ret_ty: ast::TyNode) -> ast::FuncDefine {
        let mut func =
            match ast::FuncDefine::new(name.to_string(), params, ret_ty, false, test_span()) {
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
        let ir = IR::new();
        let func = make_func(
            "func",
            vec![
                ArgsNode {
                    name: "self".to_string(),
                    ty: ast::TyNode::SelfTy("Name".to_string()),
                    var_attr: parse::VarMutAttr::Invar,
                    span: test_span(),
                },
                ArgsNode {
                    name: "param".to_string(),
                    ty: ast::TyNode::Ty("int".to_string()),
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
        let ir = IR::new();
        let func = make_func(
            "invalid",
            vec![
                ArgsNode {
                    name: "param".to_string(),
                    ty: ast::TyNode::Ty("int".to_string()),
                    var_attr: parse::VarMutAttr::Invar,
                    span: test_span(),
                },
                ArgsNode {
                    name: "self".to_string(),
                    ty: ast::TyNode::SelfTy("Name".to_string()),
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
        let ir = IR::new();
        let func = make_func(
            "func2",
            vec![ArgsNode {
                name: "self".to_string(),
                ty: ast::TyNode::SelfTy("Name".to_string()),
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

        let mut ir = IR::new();
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
    fn method_can_be_called_via_struct_name_scope() {
        // `self`を取らない、構造体の「静的メゾット」を定義し、
        // `Point::answer()`のように呼び出せることを確認する
        let nodes = parse_test_source(
            "struct Point { pub answer(): int { ret 42 } } main(): int { ret Point::answer() }",
        );

        let mut ir = IR::new();
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

        let mut ir = IR::new();
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
}
