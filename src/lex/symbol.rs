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
            Tkn::Equal => {
                let tkn = match &self.gen_tkns.last()?.tkn {
                    Tkn::Equal => Tkn::EqEq,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(tkn)
            },
            Tkn::Not => {
                let tkn = match &self.gen_tkns.last()?.tkn {
                    Tkn::Equal => Tkn::NotEq,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(tkn)
            }
            // `=>` (matchの条件/パターンの後に付ける矢印)
            Tkn::RAngleBracket => {
                let tkn = match &self.gen_tkns.last()?.tkn {
                    Tkn::Equal => Tkn::Arrow,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(tkn)
            },
            _ => None,
        }
    }
}

