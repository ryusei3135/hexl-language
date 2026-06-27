use super::*;


type ExprResult = Result<node::Expr, err::ErrKind>;


impl Parser {
    pub(super) fn expr_define_var(&mut self, name: String) -> ExprResult {
        let node = if self.next_tkn().is_ok() {
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
            println!("{:?}", self.current_tkn());
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
                println!("{:?}", self.current_tkn());
                return Err(err::ErrKind::UnexpectedToken);
            }
            self.next_tkn()?;
            // ここでアーム本体を解析
            let body = self.gen_block_node()?;
            // }
            if !matches!(self.current_tkn(), lex::Tkn::RBrace) {
                println!("{:?}", self.current_tkn());
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
        let v = match self.next_tkn()? {
            lex::Tkn::Number(value) => {
                node::Expr::Number(value)
            }
            lex::Tkn::Str(value) => {
                node::Expr::Str(value)
            }
            lex::Tkn::Name(name) => {
                if self.next_tkn_ref()? == &lex::Tkn::LParen {
                    self.next_tkn().unwrap();
                    let mut args = Vec::<node::Expr>::new();
                    if self.next_tkn_ref()? != &lex::Tkn::RParen {
                        loop {
                            args.push(self.expr_add()?);
                            match self.current_tkn() {
                                lex::Tkn::Comma => continue,
                                lex::Tkn::RParen => {},
                                t => panic!("{:?}", t),
                            } 
                            break;
                        }
                    } else {
                        self.next_tkn()?;
                    }
                    node::Expr::CallFunc(
                        node::CallInfo {
                            name,
                            args
                        }
                    )
                } else {
                    node::Expr::Var(name)
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
            t => panic!("{:?}", t),
        };

        match self.current_tkn() {
            lex::Tkn::Name(_) | lex::Tkn::Number(_) | lex::Tkn::Str(_) | lex::Tkn::RParen => {
                self.next_tkn()?;
            }
            _ => {},
        }

        Ok(v)
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
}
