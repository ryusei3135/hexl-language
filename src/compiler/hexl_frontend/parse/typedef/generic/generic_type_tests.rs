use crate::compiler::ast::*;
use crate::compiler::hexl_frontend::parse;
use crate::compiler::hexl_frontend::parse::test_util::{
    parse_src_zeroed as try_build, ty, zero_span as sp,
};

fn build(src: &str) -> Vec<Group1Node> {
    try_build(src).expect("parse failed")
}

/// `unsafe`共用体のメンバー(`mode`が`Unsafe`)を作る
fn unsafe_field(name: &str, ty_name: &str) -> VariantField {
    let mut field = VariantField::new(name.to_string(), ty(ty_name), sp());
    field.mode = VariantMode::Unsafe;
    field
}

fn expr(node: ExprNode) -> Expr {
    Expr::new(node, sp())
}

fn structs(nodes: &Vec<Group1Node>) -> Vec<&StructDefine> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Group1Node::StructDefine(s) => Some(s),
            _ => None,
        })
        .collect()
}

fn variants(nodes: &Vec<Group1Node>) -> Vec<&VariantDefine> {
    nodes
        .iter()
        .filter_map(|n| match n {
            Group1Node::VariantDefine(v) => Some(v),
            _ => None,
        })
        .collect()
}

fn func<'a>(nodes: &'a Vec<Group1Node>, name: &str) -> &'a FuncDefine {
    nodes
        .iter()
        .find_map(|n| match n {
            Group1Node::FuncDefine(f) if f.name == name => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("関数{}が無い: {:?}", name, nodes))
}

// ---- 構造体 ----

#[test]
fn struct_makes_one_struct_per_type() {
    let nodes = build(
        "
        struct Pair<T> { a: T b: T }
        main(): int {
            x: Pair<int> = Pair<int> { a: 1 b: 2 }
            y: Pair<byte> = Pair<byte> { a: 1 b: 2 }
            z: Pair<int> = Pair<int> { a: 3 b: 4 }
            ret 0
        }
        ",
    );
    // main + `Pair<int>` + `Pair<byte>`(型引数の数だけ作られる。
    // `Pair<int>`は2回使っても1つだけ)
    assert_eq!(nodes.len(), 3);
    // 定義そのもの(`Pair<T>`)はノードにならない
    assert!(structs(&nodes).iter().all(|s| s.name != "Pair"));

    let ss = structs(&nodes);
    for t in ["int", "byte"] {
        let name = format!("Pair${}", t);
        let s = ss.iter().find(|s| s.name == name).expect("型が無い");
        assert_eq!(
            Group1Node::StructDefine((*s).clone()),
            StructDefine::new(
                name,
                vec![
                    StructField::new("a".to_string(), ty(t), sp()),
                    StructField::new("b".to_string(), ty(t), sp()),
                ],
                Vec::new(),
                sp()
            )
        );
    }
}

#[test]
fn struct_field_visibility_is_preserved() {
    let nodes = build(
        "
        struct Record<T> { pub visible: T hidden: T }
        f(record: Record<int>): int { ret 0 }
        ",
    );
    let fields = &structs(&nodes)[0].fields;
    assert!(fields[0].public);
    assert!(!fields[1].public);
}

#[test]
fn struct_used_before_definition() {
    // 使用が定義より前でも作れる
    let nodes = build(
        "
        main(p: Pair<int>): int { ret 0 }
        struct Pair<T> { a: T }
        ",
    );
    assert_eq!(structs(&nodes)[0].name, "Pair$int");
}

#[test]
fn uncalled_generic_struct_makes_no_node() {
    let nodes = build(
        "
        struct Pair<T> { a: T b: T }
        main(): int { ret 0 }
        ",
    );
    assert_eq!(nodes.len(), 1);
    assert_eq!(func(&nodes, "main").name, "main");
}

#[test]
fn type_in_params_and_ret_ty() {
    let nodes = build(
        "
        struct Pair<T> { a: T b: T }
        f(p: Pair<int>): Pair<int> { ret p }
        ",
    );
    let f = func(&nodes, "f");
    assert_eq!(f.params[0].ty, ty("Pair$int"));
    assert_eq!(f.ret_ty, ty("Pair$int"));
    // 2回使っても、型は1つだけ
    assert_eq!(structs(&nodes).len(), 1);
}

#[test]
fn multiple_type_params() {
    let nodes = build(
        "
        struct Pair<T, U> { a: T b: U }
        f(p: Pair<int, byte>): int { ret 0 }
        ",
    );
    let s = structs(&nodes)[0];
    assert_eq!(s.name, "Pair$int$byte");
    assert_eq!(
        s.fields,
        vec![
            StructField::new("a".to_string(), ty("int"), sp()),
            StructField::new("b".to_string(), ty("byte"), sp()),
        ]
    );
}

#[test]
fn methods_resolve_self_to_instance_name() {
    let nodes = build(
        "
        struct Box<T> {
            v: T
            new(): Self {}
            func(self: Self, param: T): T { ret 0 }
        }
        f(b: Box<int>): int { ret 0 }
        ",
    );
    let s = structs(&nodes)[0];
    assert_eq!(s.name, "Box$int");

    let Group1Node::FuncDefine(new_func) = &s.methods[0] else {
        panic!();
    };
    assert_eq!(new_func.ret_ty, TyNode::SelfTy("Box$int".to_string()));

    let Group1Node::FuncDefine(method) = &s.methods[1] else {
        panic!();
    };
    assert_eq!(method.params[0].ty, TyNode::SelfTy("Box$int".to_string()));
    assert_eq!(method.params[1].ty, ty("int"));
    assert_eq!(method.ret_ty, ty("int"));
}

#[test]
fn recursive_struct_terminates() {
    // 定義の中で自分自身(`Node<T>`)を使っていても、無限に作らない
    let nodes = build(
        "
        struct Node<T> { v: T next: Node<T>* }
        f(n: Node<int>*): int { ret 0 }
        ",
    );
    assert_eq!(structs(&nodes).len(), 1);
    let s = structs(&nodes)[0];
    assert_eq!(s.name, "Node$int");
    let TyNode::Pointer { ty_name, .. } = &s.fields[1].ty else {
        panic!("{:?}", s.fields[1]);
    };
    assert_eq!(**ty_name, ty("Node$int"));
}

#[test]
fn nested_generic_type_arg() {
    // `>>`と続けず、`> >`と書く
    let nodes = build(
        "
        struct Box<T> { v: T }
        struct Pair<T> { a: T }
        f(p: Pair<Box<int> >): int { ret 0 }
        ",
    );
    let ss = structs(&nodes);
    assert!(ss.iter().any(|s| s.name == "Box$int"));
    let pair = ss
        .iter()
        .find(|s| s.name == "Pair$Box$int")
        .expect("型が無い");
    assert_eq!(pair.fields[0].ty, ty("Box$int"));
}

// ---- バリアント ----

#[test]
fn variant_makes_one_variant_per_type() {
    let nodes = build(
        "
        variant Opt<T> { Some(T) None }
        main(): int {
            a: Opt<int> = Opt<int>::None
            b: Opt<byte> = Opt<byte>::None
            ret 0
        }
        ",
    );
    assert_eq!(nodes.len(), 3);
    let vs = variants(&nodes);
    for t in ["int", "byte"] {
        let name = format!("Opt${}", t);
        let v = vs.iter().find(|v| v.name == name).expect("型が無い");
        assert_eq!(
            v.fields,
            vec![
                VariantField::new("Some".to_string(), ty(t), sp()),
                VariantField::typeless_new("None".to_string(), sp()),
            ]
        );
    }
}

#[test]
fn variant_member_access_uses_instance_name() {
    let nodes = build(
        "
        variant Opt<T> { Some(T) None }
        main(): int {
            a: Opt<int> = Opt<int>::None
            ret 0
        }
        ",
    );
    let main = func(&nodes, "main");
    let Group2Node::Expr(Expr {
        node: ExprNode::DefVar(v),
        ..
    }) = main.body[0].get_node()
    else {
        panic!("{:?}", main.body[0]);
    };
    assert_eq!(
        *v.value,
        expr(ExprNode::EnumVariant {
            name: "Opt$int".to_string(),
            variant: "None".to_string(),
        })
    );
}

#[test]
fn generic_variant_constructor_is_init_variant_node() {
    let nodes = build(
        "
        variant Option<T> { Some(T) None }
        main(): int {
            value: int = 10
            o: Option<int> = Option::Some(value)
            ret 0
        }
        ",
    );
    let main = func(&nodes, "main");
    let Group2Node::Expr(Expr {
        node: ExprNode::DefVar(var),
        ..
    }) = main.body[1].get_node()
    else {
        panic!("{:?}", main.body[1]);
    };
    assert_eq!(
        *var.value,
        expr(ExprNode::InitVariant {
            is_self: false,
            name: "Option".to_string(),
            fields: [(
                "Some".to_string(),
                Box::new(expr(ExprNode::Var("value".to_string())))
            )]
            .into_iter()
            .collect(),
        })
    );
}

#[test]
fn unsafe_variant() {
    let nodes = build(
        "
        variant \"unsafe\" Raw<T> { a: T b: i64 }
        f(r: Raw<int>): int { ret 0 }
        ",
    );
    let v = variants(&nodes)[0];
    assert_eq!(v.name, "Raw$int");
    assert_eq!(
        v.fields,
        vec![unsafe_field("a", "int"), unsafe_field("b", "i64"),]
    );
    assert!(!v.is_tagged());
}

// ---- 関数のジェネリクスとの組み合わせ ----

#[test]
fn generic_func_uses_generic_type() {
    let nodes = build(
        "
        struct Pair<T> { a: T }
        f<T>(p: Pair<T>): T { ret 0 }
        main(): int {
            x: Pair<int> = Pair<int> { a: 1 }
            ret f<int>(x)
        }
        ",
    );
    let generated = nodes
        .iter()
        .find_map(|n| match n {
            Group1Node::FuncDefine(f) if f.name == "f" => Some(f),
            _ => None,
        })
        .expect("f<int>が無い");
    assert_eq!(generated.params[0].ty, ty("Pair$int"));
    assert_eq!(structs(&nodes).len(), 1);
}

// ---- エラー ----

#[test]
fn too_many_type_args_is_err() {
    assert!(
        try_build(
            "
        struct Pair<T> { a: T }
        f(p: Pair<int, int>): int { ret 0 }
        "
        )
        .is_err()
    );
}

#[test]
fn too_few_type_args_is_err() {
    assert!(
        try_build(
            "
        struct Pair<T, U> { a: T b: U }
        f(p: Pair<int>): int { ret 0 }
        "
        )
        .is_err()
    );
}

#[test]
fn duplicate_type_param_is_err() {
    assert!(try_build("struct Pair<T, T> { a: T }").is_err());
}

#[test]
fn generic_alias_expands_to_instance_type() {
    let nodes = build(
        "
        struct Pair<T> { a: T b: T }
        type IntPair<T> = Pair<T>
        f(p: IntPair<int>): int { ret 0 }
        ",
    );
    assert_eq!(func(&nodes, "f").params[0].ty, ty("Pair$int"));
    // エイリアスはノードを作らず、使われた型の構造体だけができる
    assert_eq!(structs(&nodes).len(), 1);
}

#[test]
fn generic_alias_pointer_and_identity() {
    let nodes = build(
        "
        type Id<T> = T
        f(p: Id<int>): int { ret 0 }
        ",
    );
    assert_eq!(func(&nodes, "f").params[0].ty, ty("int"));
}

#[test]
fn generic_alias_wrong_arg_count_is_err() {
    assert!(
        try_build(
            "
        type Id<T> = T
        f(p: Id<int, byte>): int { ret 0 }
        "
        )
        .is_err()
    );
}

#[test]
fn non_generic_type_with_type_args_is_err() {
    assert!(
        try_build(
            "
        struct Plain { a: int }
        f(p: Plain<int>): int { ret 0 }
        "
        )
        .is_err()
    );
}

#[test]
fn infinitely_growing_type_is_err() {
    // `Node<T>`の中で`Node<T*>`を使うと、型が増え続ける
    assert!(
        try_build(
            "
        struct Node<T> { next: Node<T*> }
        f(n: Node<int>): int { ret 0 }
        "
        )
        .is_err()
    );
}

// ---- 型名 ----

#[test]
fn mangle_names() {
    assert_eq!(
        parse::Parser::mangle_generic_name("Pair", &[ty("int"), ty("byte")]),
        "Pair$int$byte"
    );
    assert_eq!(
        parse::Parser::mangle_generic_name(
            "Box",
            &[TyNode::Pointer {
                is_const: false,
                ty_name: Box::new(ty("int")),
                range: None,
            }]
        ),
        "Box$int_ptr"
    );
}
