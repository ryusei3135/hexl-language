use super::*;



impl Lexer {
    pub(super) fn join_sym_tkn(&mut self, curr_tkn: &LocatedTkn) -> Option<Tkn> {
        match &curr_tkn.tkn {
            Tkn::Colon => {
                let tkn = match &self.gen_tkns.last()?.tkn {
                    Tkn::Colon => Tkn::ModPathTkn,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(tkn)
            },
            _ => None,
        }
    }
}

