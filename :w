use super::*;


impl Parser {
    pub(super) fn build_scope_node(
        &mut self,
        name: &String
    ) -> Result<Option<node::Group2Node>, err::ErrKind> {
        // "::"がないので、何も返さない
        if !matches!(self.next_tkn_ref()?, lex::Tkn::ModPathTkn) {
            return Ok(None);
        }
        // "::"をスキップ
        let _ = self.next_tkn()?;
        let mut path_node = Vec::<String>::new();
        loop {
            if let lex::Tkn::Name(name) = self.next_tkn()? {
                path_node.push(name);
            } else {
                panic!();
            }

            if !matches!(self.next_tkn_ref()?, lex::Tkn::ModPathTkn) {
                let node = node::Group2Node::scope {
                    scope: path_node,
                    target: Box::new(
                        node::Group2Node::Expr(
                        self.expr_define_var(
                            path_node
                            .last()
                            .unwrap()
                            .to_string()
                        )?
                    ))
                };
                Ok(Some(node))
            } else {
                let _ = self.next_tkn()?;
            }
        }
    }

    pub(super) fn build_member_node(&mut self) -> Result<node, err::ErrKind>
}
