use super::*;



impl Parser {
    pub fn make_range_node(
        &mut self,
        base_ty: node::TyNode,
    ) -> Result<node::TyNode, err::ErrKind> {
        if !matches!(self.current_tkn(), lex::Tkn::Mul) {
            panic!();
        }
        let left = match self.advance_tkn().unwrap() {
            lex::Tkn::Number(val) => val.parse::<usize>().unwrap(),
            _ => panic!(),
        };

        if matches!(
            self.advance_tkn().unwrap(), 
            lex::Tkn::RangeTkn
        ) {
            match self.advance_tkn().unwrap() {
                lex::Tkn::Number(val) => {
                    if !matches!(
                        self.advance_tkn().unwrap(), 
                        lex::Tkn::RBracket
                    ) {
                        panic!();
                    }
                    self.advance_tkn().unwrap();
                    return Ok(node::TyNode::Pointer {
                        is_const: false,
                        ty_name: Box::new(base_ty),
                        range: Some(
                            (left, val.parse::<usize>().unwrap())
                        ),
                    });
                }
                _ => panic!(),
            }
        }
        Err(err::ErrKind::UnexpectedToken)
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
                    is_const: false,
                    ty_name: Box::new(node::TyNode::Ty("int".to_string())),
                    range: Some((1, 10)),
                },
                is_mut: false,
            }))
        );
    }
}
