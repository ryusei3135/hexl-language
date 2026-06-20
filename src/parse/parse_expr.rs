use super::*;


type ExprResult = Result<node::Expr, err::ErrKind>;


impl Parser {
    pub(super) fn expr_define_var(&mut self, name: String) -> ExprResult {
        println!("{:?}", self.current_tkn());
        let node = if self.next_tkn().is_ok() {
            let ty_node = if self.current_tkn() == &lex::Tkn::Colon {
                self.define_ty_node()?
            } else {
                panic!("{:?}", self.current_tkn());
            };

            if self.current_tkn() == &lex::Tkn::Equal {
                node::DefineVar {
                    name: name.clone(),
                    value: Box::new(self.expr_add()?),
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
