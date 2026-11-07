use super::*;

/// `data"len"`を関数の引数にすると、`Str`(`M0`)と対になる`StrLen`(`M0Len`)を参照する
#[test]
fn str_len_arg_refers_to_static_len() {
    let body = build_func_body(
        "write(p: [b1 11], n: uint64): int { ret 0 } \
         _start(): int { data: [b1 11] = \"hello world\" write(data, data\"len\") ret 0 }",
    );
    let str_id = body
        .iter()
        .position(|i| matches!(i, inst::Inst::Str { .. }));
    let str_len = body.iter().find_map(|i| match i {
        inst::Inst::StrLen { str_id, len, .. } => Some((*str_id, *len)),
        _ => None,
    });
    assert_eq!(str_len, str_id.map(|id| (id, 11)), "{:?}", body);
}

#[test]
fn resolves_function_return_sizes_from_user_type_definitions() {
    let nodes = parse_test_source(
        "struct Point { x: int } \
         make_point(): Point { ret Point { x: 1 } } \
         variant Color { Red Green(int) } \
         make_color(): Color { ret Color::Green(1) } \
         variant Shade { Red Blue } \
         make_shade(): Shade { ret Shade::Red }",
    );
    let mut ir = new_test_ir();
    ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
        .unwrap();

    assert_eq!(
        ir.func_tree.get("make_point", None).unwrap().get_ret_ty(),
        None
    );
    assert!(matches!(
        ir.func_tree.get("make_color", None).unwrap().get_ret_ty(),
        Some(types::Size::Variant { .. })
    ));
    assert_eq!(
        ir.func_tree.get("make_shade", None).unwrap().get_ret_ty(),
        Some(types::Size::DD)
    );
}
