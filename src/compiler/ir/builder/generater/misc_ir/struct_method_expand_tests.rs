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
