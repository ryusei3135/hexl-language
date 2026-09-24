use super::*;
use crate::lex;

impl Parser {
    /// 契約元の型を作成する関数
    /// ```
    /// var: T must=func
    /// ```
    #[inline(always)]
    fn ty_constract_must(&mut self, base_ty: node::TyNode) -> Result<node::TyNode, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::KeyWordMust {
            panic!("{:?}", self.current_tkn());
        }

        // `must`の後ろは必ず`=`
        let found = self.next_tkn(&["="])?;
        if found != lex::Tkn::Equal {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::MissingEqualsAfterMust { found }
            )?;
        }
        // 3. `advance_tkn` の戻り値を `match` で安全かつスマートに分解
        match self.advance_tkn() {
            Some(lex::Tkn::Name(val)) => {
                let ty =
                    node::ConstractTy::new::<{ node::IS_MUST }>(Some(val), None, Box::new(base_ty));
                Ok(ty)
            }
            // `Name` 以外が来た、または EOF の場合（パニックさせずエラーを返す）
            Some(other_tkn) => {
                panic!();
            }
            None => {
                crate::syntax_err!(
                    self.build_err_span(),
                    err::SyntaxErrKind::TknIsEof {
                        expected: vec!["Name"]
                    }
                )
            }
        }
    }

    /// 契約を終了する関数の型
    /// ```
    /// func(var: T of Func)
    /// ```
    #[inline(always)]
    fn ty_constract_of(&mut self, base_ty: node::TyNode) -> Result<node::TyNode, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::KeyWordOf {
            panic!();
        }

        match self.next_tkn(&["name"])? {
            lex::Tkn::Name(val) => {
                let ty: node::TyNode =
                    node::ConstractTy::new::<{ node::IS_OF }>(None, Some(val), Box::new(base_ty));
                Ok(ty)
            }
            found => crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::MissingIdentAfterOf { found }
            )?,
        }
    }

    pub(in crate::parse) fn is_constract_ty(
        &mut self,
        base_ty: node::TyNode,
    ) -> Result<node::TyNode, err::ErrKind> {
        match self.next_tkn_ref(&[])? {
            lex::Tkn::KeyWordMust => {
                self.advance_tkn().unwrap();
                self.ty_constract_must(base_ty)
            }
            lex::Tkn::KeyWordOf => {
                self.advance_tkn().unwrap();
                self.ty_constract_of(base_ty)
            }
            _ => Ok(base_ty),
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
    fn check_constract_must() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes("main(): b1 { a: int must=a = 10 }");
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0].get_node(),
            &node::Group2Node::Expr(node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::Number("10".to_string())),
                ty: node::ConstractTy::new::<{ node::IS_MUST }>(
                    Some("a".to_string()),
                    None,
                    Box::new(node::TyNode::Ty("int".to_string()))
                ),
                is_mut: false,
            }))
        );
    }
}
