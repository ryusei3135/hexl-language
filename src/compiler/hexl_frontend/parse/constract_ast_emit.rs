use super::*;
use crate::compiler::hexl_frontend::lex;

impl Parser {
    /// 契約元の型を作成する関数
    /// ```text
    /// var: T must=func
    /// ```
    #[inline(always)]
    fn ty_constract_must(&mut self, base_ty: TyNode) -> Result<TyNode, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::KeyWordMust {
            return crate::err_at!(self.expected_at_current("must"));
        }

        // `must`の後ろは必ず`=`
        let found = self.next_tkn(&["="])?;
        if found != lex::Tkn::Equal {
            return crate::err_at!(self.missing_equals_after_must(found));
        }
        // 3. `advance_tkn` の戻り値を `match` で安全かつスマートに分解
        match self.advance_tkn() {
            Some(lex::Tkn::Name(val)) => {
                let ty = ConstractTy::new::<{ IS_MUST }>(Some(val), None, Box::new(base_ty));
                Ok(ty)
            }
            // `Name` 以外が来た、または EOF の場合（パニックさせずエラーを返す）
            Some(other_tkn) => {
                crate::err_at!(self.expected_tkn("name", other_tkn))
            }
            None => {
                crate::err_at!(self.must_name_is_eof())
            }
        }
    }

    /// 契約を終了する関数の型
    /// ```text
    /// func(var: T of Func)
    /// ```
    #[inline(always)]
    fn ty_constract_of(&mut self, base_ty: TyNode) -> Result<TyNode, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::KeyWordOf {
            return crate::err_at!(self.expected_at_current("of"));
        }

        match self.next_tkn(&["name"])? {
            lex::Tkn::Name(val) => {
                let ty: TyNode = ConstractTy::new::<{ IS_OF }>(None, Some(val), Box::new(base_ty));
                Ok(ty)
            }
            found => return crate::err_at!(self.missing_ident_after_of(found)),
        }
    }

    pub(in crate::compiler::hexl_frontend::parse) fn is_constract_ty(
        &mut self,
        base_ty: TyNode,
    ) -> Result<TyNode, err::ErrKind> {
        match self.next_tkn_ref(&[])? {
            lex::Tkn::KeyWordMust => {
                self.advance_tkn_checked()?;
                self.ty_constract_must(base_ty)
            }
            lex::Tkn::KeyWordOf => {
                self.advance_tkn_checked()?;
                self.ty_constract_of(base_ty)
            }
            _ => Ok(base_ty),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::compiler::hexl_frontend::parse;

    fn gen_nodes(content: &str) -> Vec<lex::LocatedTkn> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&content.to_string()).unwrap();
        lexer.gen_tkns.clone()
    }

    #[test]
    fn check_constract_must() {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes("main(): b1 { a: int must=a = 10 }");
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &Group2Node::Expr(Expr::new(
                ExprNode::DefVar(DefineVar {
                    name: "a".to_string(),
                    value: Box::new(Expr::new(
                        ExprNode::Number("10".to_string()),
                        std::rc::Rc::new(crate::compiler::err::Span::unknown()),
                    )),
                    ty: ConstractTy::new::<{ IS_MUST }>(
                        Some("a".to_string()),
                        None,
                        Box::new(TyNode::Ty("int".to_string()))
                    ),
                    var_attr: parse::VarMutAttr::Invar,
                }),
                std::rc::Rc::new(crate::compiler::err::Span::unknown()),
            ))
        );
    }
}