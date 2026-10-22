use super::*;
use crate::compiler::ast::{DefineVar, Expr, ExprNode, Group1Node, Group2Node, TyNode};

impl Parser {
    pub fn make_range_ptr_node(
        &mut self,
        base_ty: TyNode,
    ) -> Result<TyNode, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::Mul {
            return crate::err_at!(self.range_ptr_mul_not_found());
        }

        let mut result = (true, 0usize);
        let start_tkn = self.advance_tkn_checked()?;
        match start_tkn {
            lex::Tkn::Number(val) => {
                let Ok(start_num) = val.parse::<usize>() else {
                    return crate::err_at!(self.invalid_number(val));
                };
                result.1 = start_num;
            }
            lex::Tkn::KeyWordConst => {
                result.1 = self.get_range_start_num()?;
            }
            lex::Tkn::KeyWordMut => {
                result.0 = false;
                result.1 = self.get_range_start_num()?;
            }
            found => {
                return crate::err_at!(self.range_ptr_start_unexpected(found));
            }
        }

        if self.next_tkn(&[".."])? != lex::Tkn::RangeTkn {
            return crate::err_at!(self.range_ptr_range_tkn_not_found());
        }

        let end_tkn = self.advance_tkn_checked()?;
        let end_num = match end_tkn {
            lex::Tkn::Number(val) => match val.parse::<usize>() {
                Ok(num) => num,
                Err(_) => return crate::err_at!(self.invalid_number(val)),
            },
            found => {
                return crate::err_at!(self.range_ptr_end_not_number(found));
            }
        };

        if !matches!(self.advance_tkn_checked()?, lex::Tkn::RBracket) {
            return crate::err_at!(self.range_ptr_rbracket_not_found());
        }

        self.advance_tkn_checked()?;
        Ok(TyNode::Pointer {
            is_const: result.0,
            ty_name: Box::new(base_ty),
            range: Some((result.1, end_num)),
        })
    }

    /// 範囲付きポインタのスタート地点の数字を取得
    #[inline(always)]
    fn get_range_start_num(&mut self) -> Result<usize, err::ErrKind> {
        if let lex::Tkn::Number(val) = self.advance_tkn_checked()? {
            match val.parse::<usize>() {
                Ok(num) => Ok(num),
                Err(_) => crate::err_at!(self.invalid_number(val)),
            }
        } else {
            crate::err_at!(self.range_ptr_start_not_number())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::compiler::{err, parse};

    /// テスト用の位置。`Expr`の`PartialEq`は`span`を比較しないので、何でもよい
    fn sp() -> std::rc::Rc<err::Span> {
        std::rc::Rc::new(err::Span::new(&0, &0))
    }

    /// 位置を付けて`Expr`にする
    fn ex(n: ExprNode) -> Expr {
        Expr::new(n, sp())
    }

    fn gen_nodes(content: &str) -> Vec<lex::LocatedTkn> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&content.to_string()).unwrap();
        lexer.gen_tkns.clone()
    }

    #[test]
    fn check_ptr_range_node() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a: int[* 1..10] = [b] }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &Group2Node::Expr(ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::GetAddress(Box::new(ex(
                    ExprNode::Var("b".to_string())
                ))))),
                ty: TyNode::Pointer {
                    is_const: true,
                    ty_name: Box::new(TyNode::Ty("int".to_string())),
                    range: Some((1, 10)),
                },
                var_attr: parse::VarMutAttr::Invar,
            })))
        );
    }

    #[test]
    fn check_mut_ptr_range_node() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a: int[*mut 1..10] = [b] }");
        let Group1Node::FuncDefine(ref node) =
            p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &Group2Node::Expr(ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::GetAddress(Box::new(ex(
                    ExprNode::Var("b".to_string())
                ))))),
                ty: TyNode::Pointer {
                    is_const: false,
                    ty_name: Box::new(TyNode::Ty("int".to_string())),
                    range: Some((1, 10)),
                },
                var_attr: parse::VarMutAttr::Invar,
            })))
        );
    }

    #[test]
    fn check_invalid_ptr_range_node() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a: int[* 1..10 = [b] }");
        assert!(p.parser_for_test(&tkns).is_err());
    }
}
