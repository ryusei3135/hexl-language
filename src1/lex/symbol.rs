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
            // `=` は直前が `=` なら `==`、直前が `!` なら `!=` として結合する。
            // 修正前は `!=` の判定が誤って `Tkn::Not` 側（2文字目が `!` の場合）
            // に書かれており、2文字目として `=` が来たときに直前の `Not` と
            // 結合できず、`!=` が `Not, Equal` という別々のトークンに
            // 分かれてしまっていた。
            Tkn::Equal => {
                let tkn = match &self.gen_tkns.last()?.tkn {
                    Tkn::Equal => Tkn::EqEq,
                    Tkn::Not => Tkn::NotEq,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(tkn)
            },
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

