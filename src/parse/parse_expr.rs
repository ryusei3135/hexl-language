use super::*;


type ExprResult = Result<node::Expr, err::ErrKind>;


impl Parser {
    pub(super) fn expr_add(&mut self) -> ExprResult {
        let left = self.expr_mul()?;

        // expr_mulですでにトークンを進めているので現在のトークンを参照
        let result = match self.current_tkn() {
            lex::Tkn::Add => {
                node::Expr::Add(node::Expr::wrap(left, self.expr_mul()?))
            },
            lex::Tkn::Sub => {
                node::Expr::Sub(node::Expr::wrap(left, self.expr_mul()?))
            }
            t => panic!("{:?}", t),
        };
        Ok(result)
    }

    fn expr_mul(&mut self) -> ExprResult {
        let left = self.expr_value()?;

        let result = match self.next_tkn()? {
            lex::Tkn::Mul => {
                node::Expr::Mul(node::Expr::wrap(left, self.expr_value()?))
            }
            lex::Tkn::Div => {
                node::Expr::Div(node::Expr::wrap(left, self.expr_value()?))
            }
            _ => left,
        };
        Ok(result)
    }

    fn expr_value(&mut self) -> ExprResult {
        // ## 値のトークンが出たら
        // - 呼び出し元で、次のトークンに進めるのでNumberやRParenがきたら終了
        let v = match self.next_tkn()? {
            lex::Tkn::Number(value) => {
                node::Expr::Number(value)
            }
            lex::Tkn::LParen => {
                self.next_tkn()?;
                let result= self.expr_add()?;

                if !self.expect_next_tkn(lex::Tkn::RParen)? {
                    Err(err::ErrKind::UnexpectedToken)?
                } else {
                    result
                }
            }
            _ => panic!(),
        };
        Ok(v)
    }
}
