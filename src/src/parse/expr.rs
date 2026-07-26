use super::*;


type ExprResult = Result<node::Expr, err::ErrKind>;


impl Parser {
    pub(super) fn expr_define_var(&mut self, name: String) -> ExprResult {
        let node = if self.next_tkn().is_ok() {
            match &self.current_tkn() {
                lex::Tkn::LParen => {
                    // 関数の呼び出しノードを生成
                    return self.call_func_expr(&name);
                }
                lex::Tkn::LBrace => {
                    return self.struct_init_node(&name);
                }
                lex::Tkn::Comma => {
                    return Ok(node::Expr::Var(name));
                }
                _ => {}
            }

            // コロンが来た場合、それは型の定義なので、変数の定義
            let ty_node = if self.current_tkn() == &lex::Tkn::Colon {
                self.define_ty_node()?
            } else {
                // define assign var node
                return Ok(node::Expr::Assign {
                    name,
                    value: Box::new(self.expr_branch()?),
                });
            };

            if self.current_tkn() == &lex::Tkn::Equal {
                node::DefineVar {
                    name: name.clone(),
                    value: Box::new(self.expr_branch()?),
                    ty: ty_node,
                }.wrap()
            } else {
                panic!();
            }
        } else {
            panic!();
        };

        Ok(node)
    }

    /// 式に代入する物が、構文の式 例(match)かどうかで
    pub(super) fn expr_branch(&mut self) -> ExprResult {
        if let Ok(lex::Tkn::KeyWordMatch) = self.next_tkn_ref() {
            self.next_tkn()?;
            self.expr_match()
        } else {
            self.expr_cmp()
        }
    }

    pub(super) fn expr_cmp(&mut self) -> ExprResult {
        let mut left = self.expr_add()?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::LAngleBracket => {
                    node::Expr::LessThen(node::Expr::wrap(left, self.expr_add()?))
                }
                lex::Tkn::RAngleBracket => {
                    node::Expr::GreaterThen(node::Expr::wrap(left, self.expr_add()?))
                }
                _ => break,
            };
        }

        Ok(left)
    }

    pub(super) fn expr_add(&mut self) -> ExprResult {
        let mut left = self.expr_mul()?;

        // expr_mulですでにトークンを進めているので現在のトークンを参照
        loop {
            left = match self.current_tkn() {
                lex::Tkn::Add => {
                    node::Expr::Add(node::Expr::wrap(left, self.expr_mul()?))
                },
                lex::Tkn::Sub => {
                    node::Expr::Sub(node::Expr::wrap(left, self.expr_mul()?))
                }
                _ => break,
            };
        }
        Ok(left)
    }

    /// 最初にキーワードのmatchが来る必要がある
    pub(super) fn expr_match(&mut self) -> ExprResult {
        // match の対象式
        let target = if matches!(self.current_tkn(), lex::Tkn::LBrace) {
            None
        } else {
            Some(Box::new(self.expr_cmp()?))
        };

        // {
        if !matches!(self.current_tkn(), lex::Tkn::LBrace) {
            return Err(err::ErrKind::UnexpectedToken);
        }

        let mut arms = Vec::new();

        loop {
            // }
            if matches!(self.current_tkn(), lex::Tkn::RBrace) {
                break;
            }
            // パターン else 
            if self.current_tkn() == &lex::Tkn::Or {
                // {
                self.next_tkn()?;
                if !matches!(self.current_tkn(), lex::Tkn::LBrace) {
                    return Err(err::ErrKind::UnexpectedToken);
                }
                self.next_tkn()?;
                // ここでアーム本体を解析
                let body = self.gen_block_node()?;
                // }
                if !matches!(self.current_tkn(), lex::Tkn::RBrace) {
                    return Err(err::ErrKind::UnexpectedToken);
                }
                // }
                if !matches!(self.next_tkn()?, lex::Tkn::RBrace) {
                    panic!("e");
                }
                //}
                self.next_tkn()?;
                let node = node::Expr::Match {
                    pattern: target,
                    arms,
                    arm_else: Some(body),
                };
                return Ok(node);
            }
            // if 
            let pattern = self.expr_cmp()?;

            // {
            if !matches!(self.current_tkn(), lex::Tkn::LBrace) {
                return Err(err::ErrKind::UnexpectedToken);
            }
            self.next_tkn()?;
            // ここでアーム本体を解析
            let body = self.gen_block_node()?;
            // }
            if !matches!(self.current_tkn(), lex::Tkn::RBrace) {
                return Err(err::ErrKind::UnexpectedToken);
            }
            self.next_tkn()?;
            arms.push(node::MatchArm {
                pattern: Box::new(pattern),
                body,
            });
        }
        self.next_tkn()?;//}

        let node = node::Expr::Match {
            pattern: target,
            arms,
            arm_else: None,
        };
        Ok(node)
    }

    fn expr_mul(&mut self) -> ExprResult {
        let mut left = self.expr_value()?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::Mul => {
                    node::Expr::Mul(node::Expr::wrap(left, self.expr_value()?))
                }
                lex::Tkn::Div => {
                    node::Expr::Div(node::Expr::wrap(left, self.expr_value()?))
                }
                _ => break,
            };
        }
        Ok(left)
    }

    fn expr_value(&mut self) -> ExprResult {
        // ## 値のトークンが出たら
        // - 呼び出し元で、次のトークンに進めるのでNumberやRParenがきたら終了
        if let lex::Tkn::Name(name) = self.current_tkn().clone() {
            if !self.next_tkn().is_ok_and(|v| &v == &lex::Tkn::LParen) {
                panic!();
            }
            if self.current_tkn() == &lex::Tkn::Dot {
                return self.build_scope_node(&name); 
            }
            // 前回のトークンが名前かつ(なので、関数を呼び出すノードを作成する
            return self.call_func_expr(&name);
        }

        let v = match self.next_tkn()? {
            lex::Tkn::Number(value) => {
                node::Expr::Number(value)
            }
            lex::Tkn::Str(value) => {
                node::Expr::Str(value)
            }
            lex::Tkn::Name(name) => {
                match self.next_tkn_ref()? {
                    lex::Tkn::Dot => {
                        return self.build_scope_node(&name);
                    }
                    // 関数を呼びだすノードを作成
                    lex::Tkn::LParen => {
                        self.call_func_expr(&name)?
                    }
                    // 構造体の初期化ノードを作成する
                    lex::Tkn::LBrace => {
                        // "{"から始まらないといけないので、次に進める
                        self.next_tkn()?;
                        return self.struct_init_node(&name);
                    }
                    // 列挙型のメンバへのアクセス: `Name::Mem`
                    lex::Tkn::ModPathTkn => {
                        // "::"を飛ばす
                        self.next_tkn()?;
                        let lex::Tkn::Name(variant) = self.next_tkn()? else {
                            panic!("列挙型のメンバ名が必要です");
                        };
                        node::Expr::EnumVariant { name, variant }
                    }
                    _ => node::Expr::Var(name)
                } 
            }
            lex::Tkn::LParen => {
                let result = self.expr_add()?;

                if self.current_tkn() == &lex::Tkn::LParen {
                    dbg!(self.current_tkn());
                    Err(err::ErrKind::UnexpectedToken)?
                } else {
                    result
                }
            }
            // 配列リテラル: `{100, 100, 100, 100}`
            lex::Tkn::LBrace => {
                let mut items = Vec::<node::Expr>::new();

                if self.next_tkn_ref()? != &lex::Tkn::RBrace {
                    loop {
                        items.push(self.expr_cmp()?);

                        match self.current_tkn() {
                            lex::Tkn::Comma => continue,
                            lex::Tkn::RBrace => break,
                            t => panic!("{:?}", t),
                        }
                    }
                }
                self.next_tkn()?;
                node::Expr::Array(items)
            }
            t => panic!("{:?}", t),
        };

        match self.current_tkn() {
            lex::Tkn::Name(_) | lex::Tkn::Number(_) | lex::Tkn::Str(_)
                | lex::Tkn::RParen | lex::Tkn::RBracket => {
                self.next_tkn()?;
            }
            _ => {},
        }

        Ok(v)
    }

    /// 関数を呼び出すノードを作成
    ///
    /// ## Panics
    /// 現在のトークンが`lex::Tkn::LParen`でないならpanicする
    ///
    /// ## Safety
    /// この関数が実行される場合、現在のトークンが`lex::Tkn::LParen`
    /// である必要がある
    fn call_func_expr(&mut self, name: &String) -> ExprResult {
        if self.current_tkn() != &lex::Tkn::LParen {
            panic!("call_func_exprを呼び出す際にLParenではない");
        }
        // 引数
        let mut args = Vec::<node::Expr>::new();

        // 関数を呼び出す式に引数がない場合は実行されない
        if self.next_tkn_ref()? != &lex::Tkn::RParen {
            loop {
                // 引数の式を取得
                args.push(self.expr_cmp()?);

                match self.current_tkn() {
                    lex::Tkn::Comma => continue,
                    lex::Tkn::RParen => {
                        // 関数の最後の部分に来たので、ループを終了する
                        break;
                    },
                    t => panic!("{:?}", t),
                } 
            }
        } 
        //
        // ')'をスキップ
        self.next_tkn()?;
        Ok(node::Expr::CallFunc(
            node::CallInfo {
                name: name.clone(),
                args
            }
        ))
    }
}


#[cfg(test)]
mod expr_tests {
    use crate::{
        lex,
        err,
        node::{self, *},
        parse,
    };

    fn gen_nodes(content: &str) -> Vec<lex::LocatedTkn> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&content.to_string()).unwrap();
        lexer.gen_tkns.clone()
    }

    #[test]
    fn check_call_func_node() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes(
            "main(): b1 { a(10, a) }"
        );
        let node::Group1Node::FuncDefine(ref node)
            = p.parser(tkns).expect("node is err")[0]
                else {
                    panic!("not func");
                };
        assert_eq!(
            &node.body[0],
            &node::Expr::CallFunc(
                node::CallInfo {
                    name: "a".to_string(),
                    args: vec![
                        node::Expr::Number("10".to_string()),
                        node::Expr::Var("a".to_string()),
                    ]
                }
            ).wrap_group2()
        );
    }

    #[test]
    fn check_stack_var_single() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes(
            "main(): int { a: [int] = 100 }"
        );
        let node::Group1Node::FuncDefine(ref node)
            = p.parser(tkns).expect("node is err")[0]
                else {
                    panic!("not func");
                };
        assert_eq!(
            node.body[0],
            node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::Number("100".to_string())),
                ty: node::TyNode::Stack { name: "int".to_string(), len: 1 },
            }).wrap_group2()
        );
    }

    #[test]
    fn check_stack_var_array() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes(
            "main(): int { a: [int 4] = {100, 100, 100, 100} }"
        );
        let node::Group1Node::FuncDefine(ref node)
            = p.parser(tkns).expect("node is err")[0]
                else {
                    panic!("not func");
                };
        assert_eq!(
            node.body[0],
            node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::Array(vec![
                    node::Expr::Number("100".to_string()),
                    node::Expr::Number("100".to_string()),
                    node::Expr::Number("100".to_string()),
                    node::Expr::Number("100".to_string()),
                ])),
                ty: node::TyNode::Stack { name: "int".to_string(), len: 4 },
            }).wrap_group2()
        );
    }

    #[test]
    fn check_static_var_single() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes(
            "main(): int { a: \"\"[int] = 100 }"
        );
        let node::Group1Node::FuncDefine(ref node)
            = p.parser(tkns).expect("node is err")[0]
                else {
                    panic!("not func");
                };
        assert_eq!(
            node.body[0],
            node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::Number("100".to_string())),
                ty: node::TyNode::Static { name: "int".to_string(), len: 1 },
            }).wrap_group2()
        );
    }

    #[test]
    fn check_match_expr() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes(
            "
            main(): b1 {
              match {
                10 < 10 {
                  hh: b1 = 100
                }
                | {
                  a: b1 = 10
                }
              }
            }
            "
        );
        let node::Group1Node::FuncDefine(ref node)
            = p.parser(tkns).expect("node is err")[0]
                else {
                    panic!("not func");
                };
        assert_eq!(
            node.body[0],
            node::Expr::Match {
                pattern: None,
                arms: vec![
                    node::MatchArm {
                        pattern: Box::new(wrap_expr_cmp("10", "10")),
                        body: vec![
                            gen_var_node("hh", "100", "b1"),
                        ],
                    }
                ],
                arm_else: Some(
                    vec![
                        gen_var_node("a", "10", "b1"),
                    ]
                ),
            }.wrap_group2()
        );
    }

    #[test]
    fn check_enum_variant_expr() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes(
            "main(): int { a: Color = Color::Green }"
        );
        let node::Group1Node::FuncDefine(ref node)
            = p.parser(tkns).expect("node is err")[0]
                else {
                    panic!("not func");
                };
        assert_eq!(
            node.body[0],
            node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::EnumVariant {
                    name: "Color".to_string(),
                    variant: "Green".to_string(),
                }),
                ty: node::TyNode::Ty("Color".to_string()),
            }).wrap_group2()
        );
    }
}
