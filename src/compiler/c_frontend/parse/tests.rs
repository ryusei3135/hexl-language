use super::*;
use crate::compiler::ast::*;
use crate::compiler::c_frontend::lex::Preprocessor;
use crate::compiler::err::Span;
use crate::compiler::hexl_frontend::parse::VarMutAttr;
use std::rc::Rc;

fn parse(src: &str) -> Vec<Group1Node> {
    let toks = Preprocessor::new()
        .preprocess("t.c", src)
        .expect("preprocess");
    parse_c(toks).unwrap_or_else(|e| panic!("{}", e)).nodes
}

fn parse_err(src: &str) -> ParseError {
    let toks = Preprocessor::new()
        .preprocess("t.c", src)
        .expect("preprocess");
    parse_c(toks).expect_err("エラーになるはず")
}

fn func(nodes: &[Group1Node], name: &str) -> FuncDefine {
    for n in nodes {
        if let Group1Node::FuncDefine(f) = n {
            if f.name == name {
                return f.clone();
            }
        }
    }
    panic!("関数 {} が無い", name);
}

fn num(s: &str) -> Box<Expr> {
    Box::new(expr(ExprNode::Number(s.to_string())))
}
fn var(s: &str) -> Box<Expr> {
    Box::new(expr(ExprNode::Var(s.to_string())))
}
fn span() -> Rc<Span> {
    Rc::new(Span::unknown())
}
fn expr(node: ExprNode) -> Expr {
    Expr::new(node, span())
}
fn defvar(name: &str, value: Expr, ty: &TyNode, var_attr: VarMutAttr) -> ExprNode {
    ExprNode::DefVar(DefineVar::new(&name.to_string(), value, ty, var_attr))
}
fn ty(s: &str) -> TyNode {
    TyNode::Ty(s.to_string())
}
fn exprs(f: &FuncDefine) -> Vec<Expr> {
    f.body
        .iter()
        .map(|g| match &g.node {
            Group2Node::Expr(e) => e.clone(),
            Group2Node::Stmt(StmtNode::Return(e)) => {
                expr(ExprNode::Cast((ty("RETURN"), Box::new(e.clone()))))
            }
            other => panic!("{:?}", other),
        })
        .collect()
}

#[test]
fn simple_function() {
    let n = parse("int add(int a, int b) { return a + b; }");
    let f = func(&n, "add");
    assert!(f.public);
    assert_eq!(f.ret_ty, ty("int"));
    assert_eq!(f.params.len(), 2);
    assert_eq!(f.params[0].name, "a");
    assert_eq!(f.params[0].var_attr, VarMutAttr::Var);
    assert_eq!(
        f.body[0].node,
        Group2Node::Stmt(StmtNode::Return(expr(ExprNode::Add((var("a"), var("b"))))))
    );
    assert_eq!(f.body[0].line, 1);
}

#[test]
fn static_function_is_private_and_void_params() {
    let n = parse("static void f(void) { return; }");
    let f = func(&n, "f");
    assert!(!f.public);
    assert!(f.params.is_empty());
    assert!(f.body.is_empty(), "末尾のreturn;は消える");
}

#[test]
fn types_and_pointers() {
    let n = parse("unsigned long long f(const char *s, int **pp, char *const q) { return 0; }");
    let f = func(&n, "f");
    assert_eq!(f.ret_ty, ty("unsigned long long"));
    assert_eq!(
        f.params[0].ty,
        TyNode::Pointer {
            is_const: true,
            ty_name: Box::new(ty("char")),
            range: None
        }
    );
    assert_eq!(
        f.params[1].ty,
        TyNode::Pointer {
            is_const: false,
            ty_name: Box::new(TyNode::Pointer {
                is_const: false,
                ty_name: Box::new(ty("int")),
                range: None
            }),
            range: None
        }
    );
    // `char *const q` は変数がconst
    assert_eq!(f.params[2].var_attr, VarMutAttr::Invar);
}

#[test]
fn local_variables() {
    let n = parse(
        "void f() { int x = 5; int y; const int z = 1; static int c = 0; int a[3] = {1,2,3}; }",
    );
    let e = exprs(&func(&n, "f"));
    assert_eq!(
        e[0].node,
        defvar(
            "x",
            expr(ExprNode::Number("5".into())),
            &ty("int"),
            VarMutAttr::Var
        )
    );
    assert_eq!(
        e[1].node,
        defvar(
            "y",
            expr(ExprNode::Number("0".into())),
            &ty("int"),
            VarMutAttr::Var
        )
    );
    assert_eq!(
        e[2].node,
        defvar(
            "z",
            expr(ExprNode::Number("1".into())),
            &ty("int"),
            VarMutAttr::Invar
        )
    );
    assert_eq!(
        e[3].node,
        defvar(
            "c",
            expr(ExprNode::Number("0".into())),
            &TyNode::Static {
                name: Box::new(ty("int")),
                len: 1
            },
            VarMutAttr::Var
        )
    );
    assert_eq!(
        e[4].node,
        defvar(
            "a",
            expr(ExprNode::Array(vec![
                expr(ExprNode::Number("1".into())),
                expr(ExprNode::Number("2".into())),
                expr(ExprNode::Number("3".into()))
            ])),
            &TyNode::Stack {
                name: Box::new(ty("int")),
                len: 3
            },
            VarMutAttr::Var
        )
    );
}

#[test]
fn array_len_from_initializer() {
    let n = parse("void f() { char s[] = \"ab\"; }");
    let e = exprs(&func(&n, "f"));
    let ExprNode::DefVar(d) = &e[0].node else {
        panic!()
    };
    assert_eq!(
        d.ty,
        TyNode::Stack {
            name: Box::new(ty("char")),
            len: 3
        }
    );
    assert_eq!(d.value.node, ExprNode::Str("ab".into()));
}

#[test]
fn operators() {
    let n = parse(
        "int f(int a, int b) { return a * b % 3 + (a << 2) - (b >> 1) + (a & b) + (a ^ b) + (a | b); }",
    );
    let f = func(&n, "f");
    let Group2Node::Stmt(StmtNode::Return(e)) = &f.body[0].node else {
        panic!()
    };
    // 一番外側は最後の `|`
    assert!(matches!(e.node, ExprNode::Add(..)));
}

#[test]
fn unary_ops_are_desugared() {
    let n = parse(
        "void f(int a) { int b = -a; int c = !a; int d = ~a; int e = -5; int *p = &a; int g = *p; }",
    );
    let e = exprs(&func(&n, "f"));
    let val = |i: usize| match &e[i].node {
        ExprNode::DefVar(d) => (*d.value).clone(),
        _ => panic!(),
    };
    assert_eq!(val(0).node, ExprNode::Sub((num("0"), var("a"))));
    assert_eq!(val(1).node, ExprNode::Equal((var("a"), num("0"))));
    assert_eq!(val(2).node, ExprNode::XorCalcul((var("a"), num("-1"))));
    assert_eq!(val(3).node, ExprNode::Number("-5".into()));
    assert_eq!(val(4).node, ExprNode::GetAddress(var("a")));
    assert_eq!(val(5).node, ExprNode::ConnectAddr(var("p")));
}

#[test]
fn if_else_chain_becomes_match() {
    let n = parse(
        "int f(int a) { if (a < 1) return 1; else if (a == 2) { return 2; } else return 3; }",
    );
    let e = exprs(&func(&n, "f"));
    let ExprNode::Match {
        pattern,
        arms,
        arm_else,
    } = &e[0].node
    else {
        panic!()
    };
    assert!(pattern.is_none());
    assert_eq!(arms.len(), 2);
    assert_eq!(
        arms[0].pattern.node,
        ExprNode::LessThen((var("a"), num("1")))
    );
    assert_eq!(arms[1].pattern.node, ExprNode::Equal((var("a"), num("2"))));
    assert!(arm_else.is_some());
}

#[test]
fn non_bool_condition_gets_ne_zero() {
    let n = parse("void f(int a) { if (a) { a = 1; } }");
    let e = exprs(&func(&n, "f"));
    let ExprNode::Match { arms, .. } = &e[0].node else {
        panic!()
    };
    assert_eq!(arms[0].pattern.node, ExprNode::NotEq((var("a"), num("0"))));
}

#[test]
fn while_loop() {
    let n = parse("void f(int i) { while (i < 10) { i++; } }");
    let e = exprs(&func(&n, "f"));
    let ExprNode::Loop { pattern, body } = &e[0].node else {
        panic!()
    };
    assert!(pattern.is_none());
    // 先頭は「条件の否定ならbreak」
    let Group2Node::Expr(Expr {
        node: ExprNode::Match { arms, .. },
        ..
    }) = &body[0].node
    else {
        panic!()
    };
    assert_eq!(
        arms[0].pattern.node,
        ExprNode::GreaterThenEq((var("i"), num("10")))
    );
    assert!(matches!(
        arms[0].body[0].node,
        Group2Node::Stmt(StmtNode::Break(_))
    ));
    assert_eq!(
        body[1].node,
        Group2Node::Expr(expr(ExprNode::Inc(var("i"))))
    );
}

#[test]
fn infinite_loop_has_no_guard() {
    let n = parse("void f() { while (1) { break; } for (;;) { break; } }");
    let e = exprs(&func(&n, "f"));
    for x in e {
        let Expr {
            node: ExprNode::Loop { body, .. },
            ..
        } = x
        else {
            panic!()
        };
        assert_eq!(body.len(), 1);
    }
}

#[test]
fn for_loop_and_continue() {
    let n = parse("void f(int n) { for (int i = 0; i < n; i++) { if (i == 3) continue; g(i); } }");
    let e = exprs(&func(&n, "f"));
    // 初期化 + Loop
    assert!(matches!(e[0].node, ExprNode::DefVar(_)));
    let ExprNode::Loop { body, .. } = &e[1].node else {
        panic!()
    };
    // guard, if(continue), g(i), i++
    assert_eq!(body.len(), 4);
    assert_eq!(
        body[3].node,
        Group2Node::Expr(expr(ExprNode::Inc(var("i"))))
    );
    // continueの前に更新式が入る
    let Group2Node::Expr(Expr {
        node: ExprNode::Match { arms, .. },
        ..
    }) = &body[1].node
    else {
        panic!()
    };
    assert_eq!(arms[0].body.len(), 2);
    assert_eq!(
        arms[0].body[0].node,
        Group2Node::Expr(expr(ExprNode::Inc(var("i"))))
    );
    assert_eq!(arms[0].body[1].node, Group2Node::Stmt(StmtNode::Continue));
}

#[test]
fn two_for_loops_with_same_var_get_renamed() {
    let n = parse("void f() { for (int i=0;i<3;i++) {} for (int i=0;i<3;i++) {} }");
    let e = exprs(&func(&n, "f"));
    let names: Vec<String> = e
        .iter()
        .filter_map(|x| {
            if let ExprNode::DefVar(d) = &x.node {
                Some(d.name.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(names.len(), 2);
    assert_ne!(names[0], names[1]);
    // 2つ目のループ内の`i`も改名後の名前を指す
    let ExprNode::Loop { body, .. } = &e[3].node else {
        panic!()
    };
    let Group2Node::Expr(Expr {
        node: ExprNode::Match { arms, .. },
        ..
    }) = &body[0].node
    else {
        panic!()
    };
    assert_eq!(
        arms[0].pattern.node,
        ExprNode::GreaterThenEq((var(&names[1]), num("3")))
    );
}

#[test]
fn do_while() {
    let n = parse("void f(int i) { do { i--; } while (i > 0); }");
    let e = exprs(&func(&n, "f"));
    let ExprNode::Loop { body, .. } = &e[0].node else {
        panic!()
    };
    assert_eq!(
        body[0].node,
        Group2Node::Expr(expr(ExprNode::Dec(var("i"))))
    );
    let Group2Node::Expr(Expr {
        node: ExprNode::Match { arms, .. },
        ..
    }) = &body[1].node
    else {
        panic!()
    };
    assert_eq!(
        arms[0].pattern.node,
        ExprNode::LessThenEq((var("i"), num("0")))
    );
}

#[test]
fn switch_with_fallthrough_and_default() {
    let n = parse(
        "int f(int a) { int r = 0; switch (a) { case 1: case 2: r = 1; break; case 3: r = 2; default: r = 3; break; } return r; }",
    );
    let e = exprs(&func(&n, "f"));
    let ExprNode::Match {
        pattern,
        arms,
        arm_else,
    } = &e[1].node
    else {
        panic!()
    };
    assert_eq!(
        pattern.as_ref().unwrap().node,
        ExprNode::Var("a".to_string())
    );
    // case 1 / case 2 / case 3
    assert_eq!(arms.len(), 3);
    assert_eq!(*arms[0].pattern, *num("1"));
    assert_eq!(arms[0].body.len(), 1);
    assert_eq!(arms[0].body, arms[1].body);
    // case 3 は default へ落ちる: r=2; r=3;
    assert_eq!(arms[2].body.len(), 2);
    assert_eq!(arm_else.as_ref().unwrap().len(), 1);
}

#[test]
fn break_in_middle_of_switch_is_unsupported() {
    let e = parse_err("void f(int a, int b) { switch (a) { case 1: if (b) break; g(); break; } }");
    assert_eq!(e.kind, ParseErrorKind::Unsupported);
}

#[test]
fn logical_ops() {
    let n = parse(
        "int f(int a, int b, int *p) { int x = a < 1 && b < 2; int y = p != 0 && *p > 1; int z = a || b; return 0; }",
    );
    let e = exprs(&func(&n, "f"));
    let val = |i: usize| match &e[i].node {
        ExprNode::DefVar(d) => (*d.value).clone(),
        _ => panic!(),
    };
    // 右辺が安全 -> ビット演算
    assert!(matches!(val(0).node, ExprNode::AndCalcul(_)));
    // 右辺が`*p` -> 短絡評価を保つためMatch
    assert!(matches!(val(1).node, ExprNode::Match { .. }));
    // 整数は`!= 0`に直して or
    assert_eq!(
        val(2).node,
        ExprNode::OrCalcul((
            Box::new(expr(ExprNode::NotEq((var("a"), num("0"))))),
            Box::new(expr(ExprNode::NotEq((var("b"), num("0")))))
        ))
    );
}

#[test]
fn ternary() {
    let n = parse("int f(int a) { return a > 0 ? 1 : 2; }");
    let f = func(&n, "f");
    let Group2Node::Stmt(StmtNode::Return(Expr {
        node: ExprNode::Match { arms, arm_else, .. },
        ..
    })) = &f.body[0].node
    else {
        panic!()
    };
    assert_eq!(arms.len(), 1);
    assert!(arm_else.is_some());
}

#[test]
fn assignment_and_compound() {
    let n = parse("void f(int a, int *p, int *q) { a = 1; a += 2; *p = 3; q[1] = 4; p[a] <<= 1; }");
    let e = exprs(&func(&n, "f"));
    assert_eq!(
        e[0].node,
        ExprNode::Assign(AssignVar {
            name: "a".into(),
            dst: var("a"),
            value: num("1"),
        })
    );
    assert_eq!(
        e[1].node,
        ExprNode::Assign(AssignVar {
            name: "a".into(),
            dst: var("a"),
            value: Box::new(expr(ExprNode::Add((var("a"), num("2"))))),
        })
    );
    assert_eq!(
        e[2].node,
        ExprNode::Assign(AssignVar {
            name: "p".into(),
            dst: Box::new(expr(ExprNode::ConnectAddr(var("p")))),
            value: num("3"),
        })
    );
    let ExprNode::Assign(a) = &e[3].node else {
        panic!()
    };
    assert_eq!(a.name, "q");
    assert!(matches!(a.dst.node, ExprNode::RefArray { .. }));
}

#[test]
fn postfix_value_is_unsupported_but_statement_is_ok() {
    parse("void f(int i) { i++; i--; for (;;i++) { break; } }");
    let e = parse_err("void f(int i, int *a) { a[i++] = 1; }");
    assert_eq!(e.kind, ParseErrorKind::Unsupported);
}

#[test]
fn calls_and_literals() {
    let n = parse("void f() { g(1, 2.5, 'a', \"hi\\n\"); }");
    let e = exprs(&func(&n, "f"));
    assert_eq!(
        e[0].node,
        ExprNode::CallFunc(CallInfo {
            name: "g".into(),
            temp_ty: vec![],
            args: vec![
                expr(ExprNode::Number("1".into())),
                expr(ExprNode::Float("2.5".into())),
                expr(ExprNode::Number("97".into())),
                expr(ExprNode::Str("hi\n".into()))
            ]
        })
    );
}

#[test]
fn cast() {
    let n = parse("long f(int a) { return (long)a; }");
    let f = func(&n, "f");
    assert_eq!(
        f.body[0].node,
        Group2Node::Stmt(StmtNode::Return(expr(ExprNode::Cast((
            ty("long"),
            var("a")
        )))))
    );
}

#[test]
fn struct_definition_and_member_access() {
    let n = parse(
        "struct P { int x; int y; char name[8]; };
         int f(struct P *p, struct P q, struct P *arr) { q.x = 1; p->y = 2; return arr[3].x + p->x; }",
    );
    let Group1Node::StructDefine(s) = &n[0] else {
        panic!()
    };
    assert_eq!(s.name, "P");
    assert_eq!(s.fields.len(), 3);
    assert_eq!(
        s.fields[2].ty,
        TyNode::Stack {
            name: Box::new(ty("char")),
            len: 8
        }
    );
    let f = func(&n, "f");
    assert_eq!(
        f.params[0].ty,
        TyNode::Pointer {
            is_const: false,
            ty_name: Box::new(ty("P")),
            range: None
        }
    );
    let es = exprs(&f);
    let ExprNode::Assign(a) = &es[0].node else {
        panic!()
    };
    assert_eq!(
        a.dst.node,
        ExprNode::Member {
            scope: vec!["q".into()],
            target: var("x")
        }
    );
    let ExprNode::Assign(a) = &es[1].node else {
        panic!()
    };
    assert_eq!(
        a.dst.node,
        ExprNode::PtrMember {
            name: "p".into(),
            index: None,
            else_idx: None,
            target: var("y")
        }
    );
    let ExprNode::Cast((_, ret)) = &es[2].node else {
        panic!()
    };
    let ExprNode::Add((l, _)) = &ret.node else {
        panic!()
    };
    assert_eq!(
        l.node,
        ExprNode::PtrMember {
            name: "arr".into(),
            index: Some(num("3")),
            else_idx: None,
            target: var("x")
        }
    );
}

#[test]
fn nested_member_access() {
    let n = parse(
        "struct A { int v; }; struct B { struct A a; }; int f(struct B b, struct B *p) { return b.a.v + p->a.v; }",
    );
    let f = func(&n, "f");
    let Group2Node::Stmt(StmtNode::Return(Expr {
        node: ExprNode::Add((l, r)),
        ..
    })) = &f.body[0].node
    else {
        panic!()
    };
    assert_eq!(
        l.node,
        ExprNode::Member {
            scope: vec!["b".into(), "a".into()],
            target: var("v")
        }
    );
    assert_eq!(
        r.node,
        ExprNode::PtrMember {
            name: "p".into(),
            index: None,
            else_idx: None,
            target: Box::new(expr(ExprNode::Member {
                scope: vec!["a".into()],
                target: var("v")
            }))
        }
    );
}

#[test]
fn pointer_chain_is_unsupported() {
    let e =
        parse_err("struct N { struct N *next; int v; }; int f(struct N *p) { return p->next->v; }");
    assert_eq!(e.kind, ParseErrorKind::Unsupported);
}

#[test]
fn typedef_struct_and_enum() {
    let n = parse(
        "typedef struct { int x; } Pt; typedef enum { RED, GREEN, BLUE } Color; typedef int myint;
         myint f(Pt p, Color c) { if (c == GREEN) return p.x; return 0; }",
    );
    let Group1Node::StructDefine(s) = &n[0] else {
        panic!()
    };
    assert_eq!(s.name, "Pt");
    let Group1Node::VariantDefine(v) = &n[1] else {
        panic!()
    };
    assert_eq!(v.name, "Color");
    assert!(v.is_plain_enum());
    assert_eq!(v.tag_of("BLUE"), Some(2));
    let f = func(&n, "f");
    assert_eq!(f.ret_ty, ty("int"));
    assert_eq!(f.params[0].ty, ty("Pt"));
    assert_eq!(f.params[1].ty, ty("Color"));
    let es = exprs(&f);
    let ExprNode::Match { arms, .. } = &es[0].node else {
        panic!()
    };
    assert_eq!(
        arms[0].pattern.node,
        ExprNode::Equal((
            var("c"),
            Box::new(expr(ExprNode::EnumVariant {
                name: "Color".into(),
                variant: "GREEN".into()
            }))
        ))
    );
}

#[test]
fn enum_with_explicit_values_is_unsupported() {
    let e = parse_err("enum E { A = 1, B = 2 };");
    assert_eq!(e.kind, ParseErrorKind::Unsupported);
    // 定義順どおりなら明示してもよい
    parse("enum F { A = 0, B, C = 2 };");
}

#[test]
fn union_becomes_unsafe_variant() {
    let n = parse("union U { int i; float f; };");
    let Group1Node::VariantDefine(v) = &n[0] else {
        panic!()
    };
    assert!(!v.is_tagged());
    assert_eq!(v.fields[0].mode, VariantMode::Unsafe);
    assert_eq!(v.fields[1].ty, Some(ty("float")));
}

#[test]
fn struct_initializer() {
    let n = parse(
        "struct P { int x; int y; }; void f() { struct P a = {1, 2}; struct P b = {.y = 5}; struct P c; }",
    );
    let es = exprs(&func(&n, "f"));
    let ExprNode::DefVar(d) = &es[0].node else {
        panic!()
    };
    let ExprNode::InitStruct { name, fields, .. } = &d.value.node else {
        panic!()
    };
    assert_eq!(name, "P");
    assert_eq!(fields["x"].node, ExprNode::Number("1".into()));
    assert_eq!(fields["y"].node, ExprNode::Number("2".into()));
    let ExprNode::DefVar(d) = &es[1].node else {
        panic!()
    };
    let ExprNode::InitStruct { fields, .. } = &d.value.node else {
        panic!()
    };
    assert_eq!(fields.len(), 1);
    let ExprNode::DefVar(d) = &es[2].node else {
        panic!()
    };
    assert!(matches!(&d.value.node, ExprNode::InitStruct { fields, .. } if fields.is_empty()));
}

#[test]
fn global_variable_is_unsupported_but_prototypes_are_skipped() {
    let e = parse_err("int g = 3; int f() { return g; }");
    assert_eq!(e.kind, ParseErrorKind::Unsupported);
    let n = parse(
        "extern int errno; int printf(const char *, ...); int f(int); int f(int a) { return a; }",
    );
    assert_eq!(n.len(), 1);
}

#[test]
fn unsupported_features() {
    for src in [
        "int f(int a) { goto end; end: return a; }",
        "int f() { return sizeof(int); }",
        "int f(int (*cb)(int)) { return 0; }",
        "int f(int n, ...) { return 0; }",
        "struct S { int a : 3; };",
        "int f(int a, int b) { return (a, b); }",
    ] {
        let e = parse_err(src);
        assert_eq!(e.kind, ParseErrorKind::Unsupported, "{}", src);
    }
}

#[test]
fn syntax_errors() {
    for src in ["int f( { }", "int f() { return 1 }", "int f() { x = ; }"] {
        let e = parse_err(src);
        assert_eq!(e.kind, ParseErrorKind::Syntax, "{}", src);
    }
}

#[test]
fn macro_and_include_expansion_works_before_parse() {
    let n = parse("#define N 4\nint f() { int a[N]; return N; }");
    let es = exprs(&func(&n, "f"));
    let ExprNode::DefVar(d) = &es[0].node else {
        panic!()
    };
    assert_eq!(
        d.ty,
        TyNode::Stack {
            name: Box::new(ty("int")),
            len: 4
        }
    );
}

#[test]
fn local_struct_is_hoisted_before_function() {
    let n = parse("int f() { struct L { int a; } l; l.a = 1; return l.a; }");
    assert!(matches!(n[0], Group1Node::StructDefine(_)));
    assert!(matches!(n[1], Group1Node::FuncDefine(_)));
}

#[test]
fn array_of_arrays_and_pointer_to_array() {
    let n = parse("void f() { int m[2][3]; }");
    let es = exprs(&func(&n, "f"));
    let ExprNode::DefVar(d) = &es[0].node else {
        panic!()
    };
    assert_eq!(
        d.ty,
        TyNode::Stack {
            name: Box::new(TyNode::Stack {
                name: Box::new(ty("int")),
                len: 3
            }),
            len: 2
        }
    );
}

#[test]
fn realistic_program() {
    let src = r#"
        #define MAX 16
        typedef unsigned char u8;
        typedef struct Item { int id; u8 flags; const char *name; } Item;
        enum Kind { K_A, K_B, K_C };

        static int clamp(int v, int lo, int hi) {
            if (v < lo) return lo;
            if (v > hi) return hi;
            return v;
        }

        int sum(const int *a, int n) {
            int s = 0;
            for (int i = 0; i < n; i++) { s += a[i]; }
            return s;
        }

        int classify(Item *it, enum Kind k) {
            int r = 0;
            switch (k) {
            case K_A: r = 1; break;
            case K_B:
            case K_C: r = it->id > 3 && it->flags != 0 ? 2 : 3; break;
            default: break;
            }
            while (r < MAX) { r = r * 2; if (r == 8) break; }
            return clamp(r, 0, MAX);
        }
    "#;
    let toks = Preprocessor::new().preprocess("t.c", src).unwrap();
    let out = parse_c(toks).unwrap_or_else(|e| panic!("{}", e));
    assert_eq!(out.nodes.len(), 5);
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    let n = out.nodes;
    let Group1Node::StructDefine(s) = &n[0] else {
        panic!()
    };
    assert_eq!(s.name, "Item");
    assert_eq!(s.fields[1].ty, ty("unsigned char"));
}
