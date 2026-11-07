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
