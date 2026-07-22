use super::*;


impl Parser {
    /// モジュールのノードを作成
    pub(super) fn build_scope_node(
        &mut self,
        name: &String
    ) -> Result<node::Group2Node, err::ErrKind> {
        if matches!(self.next_tkn_ref()?, lex::Tkn::Dot) {
            return self.build_member_node(&name);
        }
        // "::"がないので、何も返さない
        if !matches!(self.next_tkn_ref()?, lex::Tkn::ModPathTkn) {
            return Ok(node::Group2Node::Expr(self.expr_define_var(name.to_string())?));
        }

        crate::scope_node!(self, ModPathTkn, Scope);
    }

    /// メゾットなどのノードを作成
    pub(super) fn build_member_node(&mut self, name: &String) -> Result<node::Group2Node, err::ErrKind> {
        // "::"がないので、何も返さない
        if !matches!(self.next_tkn_ref()?, lex::Tkn::Dot) {
            return Ok(node::Group2Node::Expr(self.expr_define_var(name.to_string())?));
        }
        crate::scope_node!(self, Dot, Member);
    }
}
