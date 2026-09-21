//! 変数や引数が不変か可変を判定するパーサーのモジュール


use super::*;

impl Parser {
    /// 変数を定義する際に、可変か不変かを判定する
    /// - 可変の場合、`true`を返す
    #[inline(always)]
    pub fn assign_expr_is_mut(
        &mut self, 
        name: &String
    ) -> Result<VarMutAttr, Result<node::Expr, err::ErrKind>> {
        let attr = match &self.current_tkn() {
            lex::Tkn::KeyWordMut => {
                self.advance_tkn().unwrap();
                VarMutAttr::Var
            }
            lex::Tkn::KeyWordConst => {
                self.advance_tkn().unwrap();
                VarMutAttr::Const
            }
            // コロンが来た場合、それは型の定義なので、変数の定義
            lex::Tkn::Colon => VarMutAttr::Invar,
            _ => {
                // define assign var node
                return Err(Ok(node::AssignVar::new(
                    &name,
                    node::Expr::Var(name.to_string()),
                    self.expr_branch().unwrap(),
                )));
            }
        };
        Ok(attr)
    }

    #[inline(always)]
    pub fn args_is_mut(
        &mut self, 
        _name: &String
    ) -> Result<VarMutAttr, err::ErrKind> {// 引数が不変か
        let attr = match self.next_tkn_ref(vec!["mut"])? {
            lex::Tkn::KeyWordMut => {
                VarMutAttr::Var
            }
            lex::Tkn::KeyWordConst => {
                VarMutAttr::Const
            }
            _ => VarMutAttr::Invar
        };
        if !matches!(attr, VarMutAttr::Invar) {
            self.next_tkn(vec![])?;
        }
        Ok(attr)
    }
}