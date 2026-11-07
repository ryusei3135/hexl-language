use super::*;
use crate::compiler::Field;
use crate::compiler::hexl_frontend::parse::test_util::{parse_src, parse_src_zeroed, zero_span as sp};
use std::collections::HashMap;

/// パース結果。すべての`span`と`Group2Info.line`を0にして返す
fn build(value: &str) -> Vec<Group1Node> {
    parse_src_zeroed(value).unwrap()
}

/// `unsafe`共用体のメンバー(`mode`が`Unsafe`)を作る
fn unsafe_field(name: &str, ty: &str) -> VariantField {
    let mut field = VariantField::new(name.to_string(), TyNode::Ty(ty.to_string()), sp());
    field.mode = VariantMode::Unsafe;
    field
}

/// 構文解析がエラーになるかどうか
fn build_is_err(value: &str) -> bool {
    parse_src(value).is_err()
}

#[test]
fn test_struct() {
    assert_eq!(
        &build("struct Name { name: ty name2: ty }"),
        &vec![StructDefine::new(
            "Name".to_string(),
            vec![
                StructField::make_field("name", "ty", sp()),
                StructField::make_field("name2", "ty", sp())
            ],
            Vec::new(),
            sp()
        )]
    );
}

/// `pub`が付いた構造体/バリアント/型エイリアスのノードは`public`になる
#[test]
fn test_pub_struct() {
    let mut expected = StructDefine::new(
        "Name".to_string(),
        vec![StructField::make_field("name", "ty", sp())],
        Vec::new(),
        sp(),
    );
    expected.set_public(true);
    assert_eq!(
        &build("pub struct Name { name: ty }"),
        &vec![expected.clone()]
    );
    // `pub`なしは非公開
    assert!(!build("struct Name { name: ty }")[0].is_public());
    assert!(expected.is_public());
}

#[test]
fn test_pub_variant() {
    let nodes = build("pub variant A { B C }");
    assert!(matches!(&nodes[0], Group1Node::VariantDefine(v) if v.public));
    let nodes = build("variant A { B C }");
    assert!(matches!(&nodes[0], Group1Node::VariantDefine(v) if !v.public));
}

#[test]
fn test_pub_type_alias() {
    let nodes = build("pub type T = int");
    assert!(matches!(&nodes[0], Group1Node::TypeAlias(t) if t.public));
    let nodes = build("type T = int");
    assert!(matches!(&nodes[0], Group1Node::TypeAlias(t) if !t.public));
}

/// `pub struct Name<T>`から作った型も公開される
#[test]
fn test_pub_generic_struct() {
    let nodes = build(
        "
        pub struct Pair<T> { a: T b: T }
        struct Priv<T> { a: T }
        main(): int {
            x: Pair<int> = Pair<int> { a: 1 b: 2 }
            y: Priv<int> = Priv<int> { a: 1 }
            ret 0
        }
        ",
    );
    let public_of = |name: &str| {
        nodes.iter().find_map(|n| match n {
            Group1Node::StructDefine(s) if s.name == name => Some(s.public),
            _ => None,
        })
    };
    assert_eq!(public_of("Pair$int"), Some(true));
    assert_eq!(public_of("Priv$int"), Some(false));
}

/// `impl`には`pub`を付けられない
#[test]
fn test_pub_impl_is_err() {
    assert!(build_is_err("struct A { a: int } pub impl A { f() {} }"));
}

#[test]
fn struct_init() {
    let mut func = FuncDefine::new(
        "main".to_string(),
        Vec::new(),
        TyNode::Ty("int".to_string()),
        false,
        sp(),
        None,
    );
    let map: HashMap<String, Box<Expr>> = [
        (
            "name".to_string(),
            Box::new(ExprNode::Number("1".to_string()).with_span(sp())),
        ),
        (
            "name2".to_string(),
            Box::new(ExprNode::Number("1".to_string()).with_span(sp())),
        ),
    ]
    .into_iter()
    .collect();
    let Group1Node::FuncDefine(ref mut f) = func else {
        panic!();
    };
    f.add(
        Group2Node::Expr(
            ExprNode::InitStruct {
                is_self: false,
                name: "Name".to_string(),
                fields: map,
            }
            .with_span(sp()),
        )
        .gen_group_info(0, sp()),
    );
    assert_eq!(
        &build("main(): int { Name { name: 1 name2: 1 } }"),
        &vec![func]
    );
}

#[test]
fn struct_method() {
    let mut f = vec![FuncDefine::new(
        "new".to_string(),
        Vec::new(),
        TyNode::Ty("ty".to_string()),
        false,
        sp(),
        None,
    )];
    let Group1Node::FuncDefine(func) = f.last_mut().unwrap() else {
        panic!();
    };
    func.self_module_name("Name".to_string());
    assert_eq!(
        &build("struct Name { a: int new(): ty {}}"),
        &vec![StructDefine::new(
            "Name".to_string(),
            vec![StructField::make_field("a", "int", sp()),],
            f,
            sp()
        )]
    );
}

#[test]
fn struct_method_self_ty() {
    // `Self`は、メゾットが定義されている構造体自身の名前へ
    // 解決される(第一引数/戻り値のどちらでも使用可能)
    let nodes = build(
        "struct Name { \
            a: int \
            new(): Self {} \
            func(self: Self, param: int): int {ret 0} \
        }",
    );
    let Group1Node::StructDefine(struct_def) = &nodes[0] else {
        panic!("構造体が定義されていません: {:?}", nodes);
    };

    let Group1Node::FuncDefine(new_func) = &struct_def.methods[0] else {
        panic!();
    };
    assert_eq!(new_func.name, "new");
    assert_eq!(new_func.ret_ty, TyNode::SelfTy("Name".to_string()));

    let Group1Node::FuncDefine(func) = &struct_def.methods[1] else {
        panic!();
    };
    assert_eq!(func.name, "func");
    assert_eq!(func.params[0].ty, TyNode::SelfTy("Name".to_string()));
    assert_eq!(func.params[0].name, "self");
    assert_eq!(func.params[1].name, "param");
    assert_eq!(func.params[1].ty, TyNode::Ty("int".to_string()));
}

#[test]
fn test_variant() {
    assert_eq!(
        &build("variant Name { A B(ty) }"),
        &vec![VariantDefine::new(
            "Name".to_string(),
            vec![
                VariantField::typeless_new("A".to_string(), sp()),
                VariantField::make_field("B", "ty", sp()),
            ],
            Vec::new(),
            sp()
        )]
    );
}

#[test]
fn test_variant_last_typeless() {
    // 最後のメンバーが型なしでも、`}`で終われる
    assert_eq!(
        &build("variant Name { A }"),
        &vec![VariantDefine::new(
            "Name".to_string(),
            vec![VariantField::typeless_new("A".to_string(), sp())],
            Vec::new(),
            sp()
        )]
    );
}

#[test]
fn test_variant_unsafe() {
    assert_eq!(
        &build("variant \"unsafe\" Name { a: int b: i64 }"),
        &vec![VariantDefine::new(
            "Name".to_string(),
            vec![unsafe_field("a", "int"), unsafe_field("b", "i64")],
            Vec::new(),
            sp()
        )]
    );
}

#[test]
fn variant_method() {
    let mut f = vec![FuncDefine::new(
        "new".to_string(),
        Vec::new(),
        TyNode::Ty("ty".to_string()),
        false,
        sp(),
        None,
    )];
    let Group1Node::FuncDefine(func) = f.last_mut().unwrap() else {
        panic!();
    };
    func.self_module_name("Name".to_string());
    assert_eq!(
        &build("variant Name { A(int) new(): ty {}}"),
        &vec![VariantDefine::new(
            "Name".to_string(),
            vec![VariantField::make_field("A", "int", sp())],
            f,
            sp()
        )]
    );
}

#[test]
fn variant_method_self_ty() {
    // 共用体でも`Self`は、定義されている共用体自身の名前へ解決される
    let nodes = build(
        "variant Name { \
            A(int) \
            new(): Self {} \
            func(self: Self, param: int): int {ret 0} \
        }",
    );
    let Group1Node::VariantDefine(variant_def) = &nodes[0] else {
        panic!("共用体が定義されていません: {:?}", nodes);
    };

    let Group1Node::FuncDefine(new_func) = &variant_def.methods[0] else {
        panic!();
    };
    assert_eq!(new_func.name, "new");
    assert_eq!(new_func.ret_ty, TyNode::SelfTy("Name".to_string()));

    let Group1Node::FuncDefine(func) = &variant_def.methods[1] else {
        panic!();
    };
    assert_eq!(func.name, "func");
    assert_eq!(func.params[0].ty, TyNode::SelfTy("Name".to_string()));
    assert_eq!(func.params[0].name, "self");
    assert_eq!(func.params[1].name, "param");
    assert_eq!(func.params[1].ty, TyNode::Ty("int".to_string()));
}

#[test]
fn variant_name_is_not_found() {
    assert!(build_is_err("variant { A }"));
}

#[test]
fn variant_lbrace_not_found() {
    assert!(build_is_err("variant Name A }"));
}

#[test]
fn enum_like_variant() {
    // 旧`enum Name {A B}`は、型なしメンバーだけの`variant`になる
    assert_eq!(
        &build("variant Name {A B}"),
        &vec![VariantDefine::new(
            "Name".to_string(),
            vec![
                VariantField::typeless_new("A".to_string(), sp()),
                VariantField::typeless_new("B".to_string(), sp()),
            ],
            Vec::new(),
            sp()
        )]
    );
}

#[test]
fn enum_like_variant_with_comma() {
    // 旧`enum`の`,`区切りも使える
    assert_eq!(&build("variant Name {A, B,}"), &build("variant Name {A B}"));
}

#[test]
fn variant_enum_like_with_method() {
    // 旧`enum`ではメゾットが捨てられていたが、統合後は保持される
    let nodes = build("variant Name { A B func(self: Self): int {ret 0} }");
    let Group1Node::VariantDefine(def) = &nodes[0] else {
        panic!("variant が定義されていません: {:?}", nodes);
    };
    assert_eq!(def.fields.len(), 2);
    assert_eq!(def.methods.len(), 1);
    assert!(def.is_plain_enum());
}

#[test]
fn variant_with_data_is_not_plain_enum() {
    let nodes = build("variant Name { A B(int) }");
    let Group1Node::VariantDefine(def) = &nodes[0] else {
        panic!();
    };
    assert!(!def.is_plain_enum());
    assert!(def.is_tagged());
}

#[test]
fn variant_unsafe_not_plain_enum() {
    let nodes = build("variant \"unsafe\" Name { a: int b: i64 }");
    let Group1Node::VariantDefine(def) = &nodes[0] else {
        panic!();
    };
    assert!(!def.is_tagged());
    assert!(!def.is_plain_enum());
}

#[test]
fn variant_unregistered_option() {
    assert!(build_is_err("variant \"safe\" Name { A }"));
}
