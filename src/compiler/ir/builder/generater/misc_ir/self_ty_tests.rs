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
