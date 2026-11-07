use crate::compiler::ast::*;
use crate::compiler::hexl_frontend::parse::test_util::{parse_src as try_build, ty};
use crate::compiler::{err, hexl_frontend::lex, hexl_frontend::parse};
use std::rc::Rc;

/// テストで組み立てるノードに付ける位置
fn sp() -> Rc<err::Span> {
    parse::Parser::new_for_test().build_err_span()
}

fn ex(node: ExprNode) -> Expr {
    node.with_span(sp())
}

fn num(value: &str) -> Expr {
    ex(ExprNode::Number(value.to_string()))
}

fn var(name: &str) -> Expr {
    ex(ExprNode::Var(name.to_string()))
}

fn build(src: &str) -> Vec<Group1Node> {
    try_build(src).expect("parse failed")
}

fn funcs(nodes: &Vec<Group1Node>) -> Vec<&FuncDefine> {
    nodes
        .iter()
        .map(|n| match n {
            Group1Node::FuncDefine(f) => f,
            t => panic!("関数ではありません: {:?}", t),
        })
        .collect()
}

/// `name`で、`<>`の中身が`temp_ty`の関数を探す
fn find<'a>(nodes: &'a Vec<Group1Node>, name: &str, temp_ty: Vec<TyNode>) -> &'a FuncDefine {
    funcs(nodes)
        .into_iter()
        .find(|f| f.name == name && f.temp_ty == temp_ty)
        .unwrap_or_else(|| panic!("{}<{:?}>が無い: {:?}", name, temp_ty, nodes))
}

fn call(name: &str, temp_ty: Vec<TyNode>, args: Vec<Expr>) -> Expr {
    ex(ExprNode::CallFunc(CallInfo {
        name: name.to_string(),
        temp_ty,
        args,
    }))
}

// ---- 仕様のサンプル ----

#[test]
fn spec_example_makes_one_func_per_type() {
    let nodes = build(
        "
        func<T>(arg: T): T {
            var: T = 10
            ret var
        }
        main(): int {
            a: int = func<int>(10)
            b: byte = func<byte>(10)
            ret a
        }
        ",
    );
    // main + `func<int>` + `func<byte>`(型の数だけ作られる)
    assert_eq!(nodes.len(), 3);

    // 定義そのもの(`func<T>`)はノードにならない
    assert!(
        funcs(&nodes)
            .iter()
            .all(|f| f.name != "func" || !f.temp_ty.is_empty())
    );

    for t in ["int", "byte"] {
        let f = find(&nodes, "func", vec![ty(t)]);
        assert_eq!(f.params.len(), 1);
        assert_eq!(f.params[0].name, "arg");
        // `T`が実際の型に置き換わっている
        assert_eq!(f.params[0].ty, ty(t));
        assert_eq!(f.ret_ty, ty(t));
        assert_eq!(f.body.len(), 2);
        assert_eq!(
            f.body[0].get_node(),
            gen_var_node("var", "10", t, 0).get_node()
        );
        assert_eq!(f.body[1].get_node(), &StmtNode::Return(var("var")).wrap());
        assert!(!f.public);
    }
}

#[test]
fn call_node_has_temp_ty() {
    let nodes = build(
        "
        func<T>(arg: T): T { ret arg }
        main(): int {
            var: int = func<int>(10)
            var2: byte = func<byte>(var)
        }
        ",
    );
    let main = find(&nodes, "main", vec![]);
    let Group2Node::Expr(Expr {
        node: ExprNode::DefVar(v),
        ..
    }) = main.body[0].get_node()
    else {
        panic!("{:?}", main.body[0]);
    };
    assert_eq!(*v.value, call("func", vec![ty("int")], vec![num("10")]));
    let Group2Node::Expr(Expr {
        node: ExprNode::DefVar(v),
        ..
    }) = main.body[1].get_node()
    else {
        panic!("{:?}", main.body[1]);
    };
    assert_eq!(*v.value, call("func", vec![ty("byte")], vec![var("var")]));
}

// ---- 作られる関数の数 ----

#[test]
fn same_type_is_made_only_once() {
    let nodes = build(
        "
        func<T>(arg: T): T { ret arg }
        main(): int {
            a: int = func<int>(1)
            b: int = func<int>(2)
            c: int = func<int>(3)
        }
        ",
    );
    assert_eq!(nodes.len(), 2);
    find(&nodes, "func", vec![ty("int")]);
}

#[test]
fn uncalled_generic_func_makes_no_node() {
    let nodes = build(
        "
        func<T>(arg: T): T { ret arg }
        main(): int { ret 0 }
        ",
    );
    assert_eq!(nodes.len(), 1);
    find(&nodes, "main", vec![]);
}

#[test]
fn call_before_definition() {
    let nodes = build(
        "
        main(): int {
            a: int = func<int>(1)
            ret a
        }
        func<T>(arg: T): T { ret arg }
        ",
    );
    assert_eq!(nodes.len(), 2);
    find(&nodes, "func", vec![ty("int")]);
}

#[test]
fn statement_level_and_ret_call() {
    let nodes = build(
        "
        func<T>(arg: T): T { ret arg }
        main(): int {
            func<int>(10)
            ret func<byte>(1)
        }
        ",
    );
    assert_eq!(nodes.len(), 3);
    let main = find(&nodes, "main", vec![]);
    assert_eq!(
        main.body[0].get_node(),
        &call("func", vec![ty("int")], vec![num("10")]).wrap_group2()
    );
    assert_eq!(
        main.body[1].get_node(),
        &StmtNode::Return(call("func", vec![ty("byte")], vec![num("1")])).wrap()
    );
    find(&nodes, "func", vec![ty("int")]);
    find(&nodes, "func", vec![ty("byte")]);
}

// ---- 定義の形 ----

#[test]
fn public_generic_func_keeps_public() {
    let nodes = build(
        "
        pub func<T>(arg: T): T { ret arg }
        main(): int { func<int>(1) }
        ",
    );
    assert!(find(&nodes, "func", vec![ty("int")]).public);
    assert!(!find(&nodes, "main", vec![]).public);
}

#[test]
fn multiple_type_params() {
    let nodes = build(
        "
        first<A, B>(a: A, b: B): A { ret a }
        main(): int {
            x: int = first<int, byte>(1, 2)
            y: int = first<byte, int>(1, 2)
        }
        ",
    );
    assert_eq!(nodes.len(), 3);
    let f = find(&nodes, "first", vec![ty("int"), ty("byte")]);
    assert_eq!(f.params[0].ty, ty("int"));
    assert_eq!(f.params[1].ty, ty("byte"));
    assert_eq!(f.ret_ty, ty("int"));
    // 型の順番が違えば、別の関数
    let f = find(&nodes, "first", vec![ty("byte"), ty("int")]);
    assert_eq!(f.params[0].ty, ty("byte"));
    assert_eq!(f.params[1].ty, ty("int"));
    assert_eq!(f.ret_ty, ty("byte"));
}

#[test]
fn pointer_type_arg() {
    let nodes = build(
        "
        func<T>(arg: T): T { ret arg }
        main(): int { func<int*>(1) }
        ",
    );
    let ptr = TyNode::Pointer {
        is_const: false,
        ty_name: Box::new(ty("int")),
        range: None,
    };
    let f = find(&nodes, "func", vec![ptr.clone()]);
    assert_eq!(f.params[0].ty, ptr);
    assert_eq!(f.ret_ty, ptr);
}

#[test]
fn omitted_ret_ty_stays_default() {
    let nodes = build(
        "
        func<T>(arg: T) { ret 0 }
        main(): int { func<byte>(1) }
        ",
    );
    let f = find(&nodes, "func", vec![ty("byte")]);
    assert_eq!(f.ret_ty, ty("int"));
    assert_eq!(f.params[0].ty, ty("byte"));
}

#[test]
fn member_named_like_type_param_is_not_replaced() {
    // `p.T`の`T`はメンバーの名前なので、型パラメータではない
    // (トークン列の置き換えを直接確認する)
    let lex_tkns = |src: &str| {
        let mut lexer = lex::Lexer::new("<test>");
        lexer.analy(&src.to_string()).unwrap();
        lexer.gen_tkns
    };
    let mut p = parse::Parser::new_for_test();
    p.tkns = Some(lex_tkns("func<T>(p: Point): T { ret p.T }"));
    p.collect_generic_funcs().unwrap();
    let generic = p.generic_funcs.get("func").unwrap().clone();

    let replaced = parse::Parser::substitute_ty_params(&generic, &vec![lex_tkns("int")]);
    let name = |s: &str| lex::Tkn::Name(s.to_string());
    assert_eq!(
        replaced.iter().map(|t| t.tkn.clone()).collect::<Vec<_>>(),
        vec![
            name("func"),
            lex::Tkn::LParen,
            name("p"),
            lex::Tkn::Colon,
            name("Point"),
            lex::Tkn::RParen,
            lex::Tkn::Colon,
            // 戻り値の型の`T`は置き換わる
            name("int"),
            lex::Tkn::LBrace,
            lex::Tkn::KeyWordRet,
            name("p"),
            lex::Tkn::Dot,
            // メンバーの`T`はそのまま
            name("T"),
            lex::Tkn::RBrace,
        ]
    );
}

// ---- ジェネリクス関数から、ジェネリクス関数を呼ぶ ----

#[test]
fn generic_calls_generic() {
    let nodes = build(
        "
        outer<T>(a: T): T { ret inner<T>(a) }
        inner<U>(b: U): U { ret b }
        main(): int { outer<byte>(1) }
        ",
    );
    assert_eq!(nodes.len(), 3);
    let outer = find(&nodes, "outer", vec![ty("byte")]);
    assert_eq!(
        outer.body[0].get_node(),
        &StmtNode::Return(call("inner", vec![ty("byte")], vec![var("a")])).wrap()
    );
    let inner = find(&nodes, "inner", vec![ty("byte")]);
    assert_eq!(inner.params[0].ty, ty("byte"));
}

#[test]
fn recursive_generic_func_terminates() {
    let nodes = build(
        "
        f<T>(n: T): T { ret f<T>(n) }
        main(): int { f<int>(1) }
        ",
    );
    assert_eq!(nodes.len(), 2);
    find(&nodes, "f", vec![ty("int")]);
}

#[test]
fn growing_recursion_is_an_error_not_a_hang() {
    // `f<T>`が`f<T*>`を呼ぶと、型が無限に増える
    assert!(
        try_build(
            "
        f<T>(n: T): T { ret f<T*>(n) }
        main(): int { f<int>(1) }
        "
        )
        .is_err()
    );
}

// ---- 既存の構文への影響 ----

#[test]
fn comparison_is_not_a_generic_call() {
    // `func`がジェネリクス関数でも、`<..>(`の形でなければ比較
    let nodes = build(
        "
        func<T>(arg: T): T { ret arg }
        main(): int {
            a: int = 1
            b: int = a < 3
            c: int = func < 3
        }
        ",
    );
    // 呼び出しが無いので、`func`の関数は作られない
    assert_eq!(nodes.len(), 1);
    let main = find(&nodes, "main", vec![]);
    let Group2Node::Expr(Expr {
        node: ExprNode::DefVar(v),
        ..
    }) = main.body[1].get_node()
    else {
        panic!("{:?}", main.body[1]);
    };
    assert_eq!(
        *v.value,
        ex(ExprNode::LessThen(
            (Box::new(var("a")), Box::new(num("3")),)
        ))
    );
}

#[test]
fn normal_func_is_unchanged() {
    let nodes = build("main(): int { ret 0 }");
    let f = find(&nodes, "main", vec![]);
    assert!(f.temp_ty.is_empty());
}

// ---- エラー ----

#[test]
fn wrong_number_of_type_args_is_an_error() {
    // 多い
    assert!(
        try_build(
            "
        func<T>(arg: T): T { ret arg }
        main(): int { func<int, byte>(1) }
        "
        )
        .is_err()
    );
    // 少ない
    assert!(
        try_build(
            "
        two<A, B>(a: A, b: B): A { ret a }
        main(): int { two<int>(1, 2) }
        "
        )
        .is_err()
    );
}

#[test]
fn malformed_definition_is_an_error() {
    // 型パラメータが空
    assert!(try_build("func<>(a: int): int { ret a } main(): int { ret 0 }").is_err());
    // 型パラメータの重複
    assert!(try_build("func<T, T>(a: T): T { ret a } main(): int { ret 0 }").is_err());
    // `>`が閉じていない
    assert!(try_build("func<T(a: T): T { ret a } main(): int { ret 0 }").is_err());
    // 本体が閉じていない
    assert!(try_build("func<T>(a: T): T { ret a").is_err());
}
