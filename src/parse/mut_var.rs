//! 変数や引数が不変か可変を判定するパーサーのモジュール


use super::*;

impl Parser {
    /// 変数を定義する際に、可変か不変かを判定する
    /// - 可変の場合、`true`を返す
    #[inline(always)]
    pub fn assign_expr_is_mut(
        &mut self, 
        name: &String
    ) -> Result<bool, Result<node::Expr, err::ErrKind>> {
        let is_mut = match &self.current_tkn() {
            lex::Tkn::KeyWordMut => {
                self.advance_tkn().unwrap();
                true
            }
            lex::Tkn::KeyWordConst => {
                self.advance_tkn().unwrap();
                false
            }
            // コロンが来た場合、それは型の定義なので、変数の定義
            lex::Tkn::Colon => false,
            _ => {
                // define assign var node
                return Err(Ok(node::AssignVar::new(
                    &name,
                    node::Expr::Var(name.to_string()),
                    self.expr_branch().unwrap(),
                )));
            }
        };
        Ok(is_mut)
    }

    #[inline(always)]
    pub fn args_is_mut(
        &mut self, 
        name: &String
    ) -> Result<bool, err::ErrKind> {// 引数が不変か
        let is_mut = matches!(
            self.next_tkn_ref(vec!["mut"])?, 
            lex::Tkn::KeyWordMut
        );
        if is_mut {
            self.next_tkn(vec![])?;
        }
        Ok(is_mut)
    }
}