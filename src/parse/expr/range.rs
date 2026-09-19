use super::*;



impl Parser {
    pub fn make_range_ptr_node(
        &mut self,
        base_ty: node::TyNode,
    ) -> Result<node::TyNode, err::ErrKind> {
        if !matches!(self.current_tkn(), lex::Tkn::Mul) {
            panic!();
        }
        let left: (bool, usize) = match self.advance_tkn().unwrap() {
            lex::Tkn::Number(val) => {
                (true, val.parse::<usize>().unwrap())
            }
            lex::Tkn::KeyWordConst => {
                let val: usize = self.get_range_start_num()?;
                (true, val)
            }
            lex::Tkn::KeyWordMut => {
                let val: usize = self.get_range_start_num()?;
                (false, val)
            }
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
                        is_const: left.0,
                        ty_name: Box::new(base_ty),
                        range: Some(
                            (left.1, val.parse::<usize>().unwrap())
                        ),
                    });
                }
                _ => panic!(),
            }
        }
        Err(err::ErrKind::UnexpectedToken)
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
}
