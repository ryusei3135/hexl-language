use super::*;


impl Parser {
    /// モジュールのノードを作成
    /// name::mod
    pub(super) fn build_scope_node(
        &mut self,
        name: &String
    ) -> Result<node::Expr, err::ErrKind> {
        if matches!(self.next_tkn_ref()?, lex::Tkn::Dot) {
            return self.build_member_node(&name);
        }
        // "::"がないので、何も返さない
        if !matches!(self.next_tkn_ref()?, lex::Tkn::ModPathTkn) {
            return Ok(self.expr_define_var(name.to_string())?);
        }

        crate::scope_node!(self, ModPathTkn, Scope, &name);
    }

    /// メゾットなどのノードを作成
    /// name.method
    pub(super) fn build_member_node(
        &mut self,
        name: &String
    ) -> Result<node::Expr, err::ErrKind> {
        // "."がないので、何も返さない
        if !matches!(self.next_tkn_ref()?, lex::Tkn::Dot) {
            return Ok(self.expr_define_var(name.to_string())?);
        }
        crate::scope_node!(self, Dot, Member, &name);
    }
}
