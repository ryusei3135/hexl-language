//! expr.rsのテストコード

use super::*;

#[cfg(test)]
mod expr_tests {
    use crate::compiler::ast::zero_span::ZeroSpan;
    use crate::compiler::{
        ast::{self, *},
        err,
        hexl_frontend::lex,
        hexl_frontend::parse,
    };

    /// テスト用の位置。`Expr`の`PartialEq`は`span`を比較しないので、何でもよい
    fn sp(line: usize) -> std::rc::Rc<err::Span> {
        std::rc::Rc::new(err::Span::new(line, 0, String::new()))
    }

    /// 位置を付けて`Expr`にする
    fn ex(n: ExprNode) -> Expr {
        Expr::new(n, sp(0))
    }

    fn gen_nodes(content: &str) -> Vec<lex::LocatedTkn> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&content.to_string()).unwrap();
        lexer.gen_tkns.clone()
    }

    #[test]
    fn check_get_address_var() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a: int* = [b] }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::GetAddress(Box::new(ex(ExprNode::Var(
                    "b".to_string()
                )))))),
                ty: TyNode::Pointer {
                    is_const: false,
                    ty_name: Box::new(TyNode::Ty("int".to_string())),
                    range: None,
                },
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_call_func_node() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a(10, a) }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::CallFunc(CallInfo {
                name: "a".to_string(),
                temp_ty: Vec::new(),
                args: vec![
                    ex(ExprNode::Number("10".to_string())),
                    ex(ExprNode::Var("a".to_string())),
                ]
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_float_literal_arg() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a(1.5, 2) }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::CallFunc(CallInfo {
                name: "a".to_string(),
                temp_ty: Vec::new(),
                args: vec![
                    ex(ExprNode::Float("1.5".to_string())),
                    ex(ExprNode::Number("2".to_string())),
                ]
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_bitwise_operator_precedence() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: int = 1 | 2 ^ 3 & 4 }");
        let Group1Node::FuncDefine(ref function) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        let Group2Node::Expr(Expr {
            node: ExprNode::DefVar(var),
            ..
        }) = function.body[0].get_node()
        else {
            panic!("not variable definition");
        };

        let expected = ex(ExprNode::OrCalcul(Expr::wrap(
            ex(ExprNode::Number("1".to_string())),
            ex(ExprNode::XorCalcul(Expr::wrap(
                ex(ExprNode::Number("2".to_string())),
                ex(ExprNode::AndCalcul(Expr::wrap(
                    ex(ExprNode::Number("3".to_string())),
                    ex(ExprNode::Number("4".to_string())),
                ))),
            ))),
        )));
        assert_eq!(var.value.as_ref(), &expected);
    }

    #[test]
    fn check_bitwise_operator_in_return() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { ret 1 & 2 }");
        let Group1Node::FuncDefine(ref function) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };

        let expected = StmtNode::Return(ex(ExprNode::AndCalcul(Expr::wrap(
            ex(ExprNode::Number("1".to_string())),
            ex(ExprNode::Number("2".to_string())),
        ))))
        .wrap();
        assert_eq!(function.body[0].get_node(), &expected);
    }

    #[test]
    fn check_stack_var_single() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: [int] = 100 }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::Number("100".to_string()))),
                ty: TyNode::Stack {
                    name: Box::new(TyNode::Ty("int".to_string())),
                    len: 1
                },
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_stack_var_array() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: [int 4] = {100, 100, 100, 100} }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::Array(vec![
                    ex(ExprNode::Number("100".to_string())),
                    ex(ExprNode::Number("100".to_string())),
                    ex(ExprNode::Number("100".to_string())),
                    ex(ExprNode::Number("100".to_string())),
                ]))),
                ty: TyNode::Stack {
                    name: Box::new(TyNode::Ty("int".to_string())),
                    len: 4
                },
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_static_var_single() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: static[int] = 100 }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::Number("100".to_string()))),
                ty: TyNode::Static {
                    name: Box::new(TyNode::Ty("int".to_string())),
                    len: 1
                },
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_match_expr() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(
            "
            _start(): int {
              cond {
                10 == 10 => {
                  hh: int = 100
                }
                else => {
                  a: int = 10
                }
              }
            }
            ",
        );
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node().clone().zeroed_spans(),
            ex(ExprNode::Match {
                pattern: None,
                arms: vec![MatchArm {
                    pattern: Box::new(wrap_eq_expr_cmp("10", "10")),
                    body: vec![gen_var_node("hh", "100", "int", 0),],
                    span: sp(0),
                }],
                arm_else: Some(vec![gen_var_node("a", "10", "int", 0),]),
            })
            .wrap_group2()
            .zeroed_spans()
        );
    }

    #[test]
    fn check_match_expr_bool() {
        // 1. Booleanを与えるパターン (単純なif/else)
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(
            "
            _start(): int {
              cond a == 10 {
                hh: int = 100
              } else {
                a: int = 10
              }
            }
            ",
        );
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node().clone().zeroed_spans(),
            ex(ExprNode::Match {
                pattern: None,
                arms: vec![MatchArm {
                    pattern: Box::new(ex(ExprNode::Equal((
                        Box::new(ex(ExprNode::Var("a".to_string()))),
                        Box::new(ex(ExprNode::Number("10".to_string()))),
                    )))),
                    body: vec![gen_var_node("hh", "100", "int", 0),],
                    span: sp(0),
                }],
                arm_else: Some(vec![gen_var_node("a", "10", "int", 0),]),
            })
            .wrap_group2()
            .zeroed_spans()
        );
    }

    #[test]
    fn check_match_expr_value() {
        // 2. 値を与えるパターン (他の言語のswitch/matchに相当)
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(
            "
            _start(): int {
              cond a {
                10 => {
                  hh: int = 100
                }
                20 => {
                  hh: int = 200
                }
                else => {
                  a: int = 10
                }
              }
            }
            ",
        );
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node().clone().zeroed_spans(),
            ex(ExprNode::Match {
                pattern: Some(Box::new(ex(ExprNode::Var("a".to_string())))),
                arms: vec![
                    MatchArm {
                        pattern: Box::new(ex(ExprNode::Number("10".to_string()))),
                        body: vec![gen_var_node("hh", "100", "int", 0),],
                        span: sp(0),
                    },
                    MatchArm {
                        pattern: Box::new(ex(ExprNode::Number("20".to_string()))),
                        body: vec![gen_var_node("hh", "200", "int", 0),],
                        span: sp(0),
                    }
                ],
                arm_else: Some(vec![gen_var_node("a", "10", "int", 0)]),
            })
            .wrap_group2()
            .zeroed_spans()
        );
    }

    #[test]
    fn check_ptr_member_assign() {
        // ポインタが指す構造体のメンバーへの代入: `[ptr].name = 10`
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { [ptr].name = 10 }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::Assign(AssignVar {
                name: "ptr".to_string(),
                dst: Box::new(ex(ExprNode::PtrMember {
                    name: "ptr".to_string(),
                    index: None,
                    else_idx: None,
                    target: Box::new(ex(ExprNode::Var("name".to_string()))),
                })),
                value: Box::new(ex(ExprNode::Number("10".to_string()))),
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_ptr_member_call() {
        // ポインタが指す構造体のメゾットの呼び出し: `[ptr].f()`
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { [ptr].f() }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::PtrMember {
                name: "ptr".to_string(),
                index: None,
                else_idx: None,
                target: Box::new(ex(ExprNode::CallFunc(CallInfo {
                    name: "f".to_string(),
                    temp_ty: Vec::new(),
                    args: Vec::new(),
                }))),
            })
            .wrap_group2()
        );
    }

    #[test]
    fn check_ptr_member_value() {
        // 式の中でポインタが指す構造体のメンバーを読み取る:
        // `a: int = [ptr].name`
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a: int = [ptr].name }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::PtrMember {
                    name: "ptr".to_string(),
                    index: None,
                    else_idx: None,
                    target: Box::new(ex(ExprNode::Var("name".to_string()))),
                })),
                ty: TyNode::Ty("int".to_string()),
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_cast_expr() {
        // キャスト: `(T)var` -> `Cast((T, Box(var)))`
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: i64 = (i64)b ret 1 }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::Cast((
                    TyNode::Ty("i64".to_string()),
                    Box::new(ex(ExprNode::Var("b".to_string()))),
                )))),
                ty: TyNode::Ty("i64".to_string()),
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_enum_variant_expr() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a: Color = Color::Green }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::EnumVariant {
                    name: "Color".to_string(),
                    variant: "Green".to_string(),
                })),
                ty: TyNode::Ty("Color".to_string()),
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    #[test]
    fn check_postfix_increment_node() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a++ }");
        let Group1Node::FuncDefine(ref func) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            func.body[0].get_node(),
            &ex(ExprNode::Inc(Box::new(ex(ExprNode::Var("a".to_string()))))).wrap_group2()
        );
    }

    #[test]
    fn check_postfix_decrement_node() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): int { a-- }");
        let Group1Node::FuncDefine(ref func) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            func.body[0].get_node(),
            &ex(ExprNode::Dec(Box::new(ex(ExprNode::Var("a".to_string()))))).wrap_group2()
        );
    }

    #[test]
    fn check_postfix_decrement_after_nested_block() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(
            "main(): int { loop a > 0 { cond { a > 5 => { break } else => { break } } a-- } }",
        );
        assert!(p.parser_for_test(&tkns).is_ok());
    }

    fn parse_value_of(src: &str) -> Expr {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(src);
        let Group1Node::FuncDefine(ref func) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        let Group2Node::Expr(Expr {
            node: ExprNode::DefVar(var),
            ..
        }) = func.body[0].get_node()
        else {
            panic!("not variable definition");
        };
        *var.value.clone()
    }

    fn var(name: &str) -> Box<Expr> {
        Box::new(ex(ExprNode::Var(name.to_string())))
    }

    #[test]
    fn check_unary_sign() {
        assert_eq!(
            parse_value_of("main(): int { a: int = -b }"),
            ex(ExprNode::Sign((false, var("b"))))
        );
        assert_eq!(
            parse_value_of("main(): int { a: int = +b }"),
            ex(ExprNode::Sign((true, var("b"))))
        );
    }

    #[test]
    fn check_unary_not_and_inversion() {
        assert_eq!(
            parse_value_of("main(): int { a: int = !b }"),
            ex(ExprNode::CmpNot(var("b")))
        );
        assert_eq!(
            parse_value_of("main(): int { a: int = ~b }"),
            ex(ExprNode::Inversion(var("b")))
        );
    }

    #[test]
    fn check_unary_prefix_inc_dec() {
        assert_eq!(
            parse_value_of("main(): int { a: int = ++b }"),
            ex(ExprNode::LeftInc(var("b")))
        );
        assert_eq!(
            parse_value_of("main(): int { a: int = --b }"),
            ex(ExprNode::LeftDec(var("b")))
        );
    }

    #[test]
    fn check_unary_nested() {
        // `!~b` / `- -b`
        assert_eq!(
            parse_value_of("main(): int { a: int = !~b }"),
            ex(ExprNode::CmpNot(Box::new(ex(ExprNode::Inversion(var(
                "b"
            ))))))
        );
        assert_eq!(
            parse_value_of("main(): int { a: int = - -b }"),
            ex(ExprNode::Sign((
                false,
                Box::new(ex(ExprNode::Sign((false, var("b")))))
            )))
        );
    }

    #[test]
    fn check_unary_precedence() {
        // `-b * c` は `(-b) * c`
        assert_eq!(
            parse_value_of("main(): int { a: int = -b * c }"),
            ex(ExprNode::Mul((
                Box::new(ex(ExprNode::Sign((false, var("b"))))),
                var("c")
            )))
        );
        // `b - -c` は 二項の`-`と単項の`-`
        assert_eq!(
            parse_value_of("main(): int { a: int = b - -c }"),
            ex(ExprNode::Sub((
                var("b"),
                Box::new(ex(ExprNode::Sign((false, var("c")))))
            )))
        );
        // `-(b + c)`
        assert_eq!(
            parse_value_of("main(): int { a: int = -(b + c) }"),
            ex(ExprNode::Sign((
                false,
                Box::new(ex(ExprNode::Add((var("b"), var("c")))))
            )))
        );
    }

    #[test]
    fn check_logical_and_or() {
        assert_eq!(
            parse_value_of("main(): int { a: int = b && c }"),
            ex(ExprNode::CmpAnd((var("b"), var("c"))))
        );
        assert_eq!(
            parse_value_of("main(): int { a: int = b || c }"),
            ex(ExprNode::CmpOr((var("b"), var("c"))))
        );
    }

    #[test]
    fn check_logical_precedence() {
        // `b || c && d` は `b || (c && d)`
        assert_eq!(
            parse_value_of("main(): int { a: int = b || c && d }"),
            ex(ExprNode::CmpOr((
                var("b"),
                Box::new(ex(ExprNode::CmpAnd((var("c"), var("d")))))
            )))
        );
        // `b && c | d` は `b && (c | d)`(ビット演算の方が優先)
        assert_eq!(
            parse_value_of("main(): int { a: int = b && c | d }"),
            ex(ExprNode::CmpAnd((
                var("b"),
                Box::new(ex(ExprNode::OrCalcul((var("c"), var("d")))))
            )))
        );
        // `!b && c` は `(!b) && c`
        assert_eq!(
            parse_value_of("main(): int { a: int = !b && c }"),
            ex(ExprNode::CmpAnd((
                Box::new(ex(ExprNode::CmpNot(var("b")))),
                var("c")
            )))
        );
        // 左結合: `b && c && d` は `(b && c) && d`
        assert_eq!(
            parse_value_of("main(): int { a: int = b && c && d }"),
            ex(ExprNode::CmpAnd((
                Box::new(ex(ExprNode::CmpAnd((var("b"), var("c"))))),
                var("d")
            )))
        );
    }

    #[test]
    fn check_ternary() {
        assert_eq!(
            parse_value_of("main(): int { a: int = b ? c : d }"),
            ex(ExprNode::Ternary((var("b"), var("c"), var("d"))))
        );
    }

    #[test]
    fn check_ternary_precedence() {
        // 条件部は`||`より低い: `b || c ? d : e` は `(b || c) ? d : e`
        assert_eq!(
            parse_value_of("main(): int { a: int = b || c ? d : e }"),
            ex(ExprNode::Ternary((
                Box::new(ex(ExprNode::CmpOr((var("b"), var("c"))))),
                var("d"),
                var("e")
            )))
        );
        // 右結合: `b ? c : d ? e : f` は `b ? c : (d ? e : f)`
        assert_eq!(
            parse_value_of("main(): int { a: int = b ? c : d ? e : f }"),
            ex(ExprNode::Ternary((
                var("b"),
                var("c"),
                Box::new(ex(ExprNode::Ternary((var("d"), var("e"), var("f")))))
            )))
        );
        // then側のネスト: `b ? c ? d : e : f`
        assert_eq!(
            parse_value_of("main(): int { a: int = b ? c ? d : e : f }"),
            ex(ExprNode::Ternary((
                var("b"),
                Box::new(ex(ExprNode::Ternary((var("c"), var("d"), var("e"))))),
                var("f")
            )))
        );
        // 括弧内でも使える
        assert_eq!(
            parse_value_of("main(): int { a: int = (b ? c : d) + e }"),
            ex(ExprNode::Add((
                Box::new(ex(ExprNode::Ternary((var("b"), var("c"), var("d"))))),
                var("e")
            )))
        );
    }
}
