use crate::err::{
    ErrKind::UnexpectedToken,
    syntax_err::SyntaxErrKind,
};

use super::*;



impl Parser {
    pub fn make_range_ptr_node(
        &mut self,
        base_ty: node::TyNode,
    ) -> Result<node::TyNode, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::Mul {
            return Err(err::ErrKind::UnexpectedToken);
        }

        let mut result = (true, 0usize);
        let start_tkn = self.advance_tkn().unwrap();
        match start_tkn {
            lex::Tkn::Number(val) => {
                result.1 = val
                    .parse::<usize>()
                    .unwrap();
            }
            lex::Tkn::KeyWordConst => {
                result.1 = self.get_range_start_num()?;
            }
            lex::Tkn::KeyWordMut => {
                result.0 = false;
                result.1 = self.get_range_start_num()?;
            }
            found => {
                return crate::syntax_err!(
                    self.build_err_span(),
                    err::SyntaxErrKind::ExpectedKind {
                        expected: "number or `const`/`mut`",
                        found,
                    }
                );
            }
        }

        if self.next_tkn(&[".."])? != lex::Tkn::RangeTkn {
            return Err(err::ErrKind::UnexpectedToken);
        }

        let end_tkn = self.advance_tkn().unwrap();
        let end_num = match end_tkn {
            lex::Tkn::Number(val) => {
                val.parse::<usize>().unwrap()
            }
            found => {
                return crate::syntax_err!(
                    self.build_err_span(),
                    err::SyntaxErrKind::ExpectedKind {
                        expected: "number",
                        found,
                    }
                );
            }
        };

        if !matches!(
            self.advance_tkn().unwrap(), 
            lex::Tkn::RBracket
        ) {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::ExpectedKind {
                    expected: "]",
                    found: self.current_tkn().clone(),
                }
            );
        }

        self.advance_tkn().unwrap();
        Ok(node::TyNode::Pointer {
            is_const: result.0,
            ty_name: Box::new(base_ty),
            range: Some((result.1, end_num)),
        })
    }

    /// 範囲付きポインタのスタート地点の数字を取得
    #[inline(always)]
    fn get_range_start_num(
        &mut self,
    ) -> Result<usize, err::ErrKind> {
        if let lex::Tkn::Number(val) = self.advance_tkn().unwrap() {
            Ok(
                val.parse::<usize>().unwrap()
            )
        } else {
            Err(err::ErrKind::UnexpectedToken)
        }
    }
}



#[cfg(test)]
mod test {
    use super::*;
    use crate::parse;

    fn gen_nodes(content: &str) -> Vec<lex::LocatedTkn> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&content.to_string()).unwrap();
        lexer.gen_tkns.clone()
    }

    #[test]
    fn check_ptr_range_node() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes("main(): b1 { a: int[* 1..10] = [b] }");
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            &node.body[0],
            &node::Group2Node::Expr(node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::GetAddress(Box::new(node::Expr::Var(
                    "b".to_string()
                )))),
                ty: node::TyNode::Pointer {
                    is_const: true,
                    ty_name: Box::new(node::TyNode::Ty("int".to_string())),
                    range: Some((1, 10)),
                },
                is_mut: false,
            }))
            .gen_group_info(&1)
        );
    }

    #[test]
    fn check_mut_ptr_range_node() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes("main(): b1 { a: int[*mut 1..10] = [b] }");
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            &node.body[0],
            &node::Group2Node::Expr(node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::GetAddress(Box::new(node::Expr::Var(
                    "b".to_string()
                )))),
                ty: node::TyNode::Pointer {
                    is_const: false,
                    ty_name: Box::new(node::TyNode::Ty("int".to_string())),
                    range: Some((1, 10)),
                },
                is_mut: false,
            }))
            .gen_group_info(&1)
        );
    }

    #[test]
    fn check_invalid_ptr_range_node() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes("main(): b1 { a: int[* 1..10 = [b] }");
        assert!(p.parser(tkns).is_err());
    }
}
