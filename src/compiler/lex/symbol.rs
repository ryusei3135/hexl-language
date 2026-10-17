use super::*;

impl Lexer {
    pub(in crate::compiler::lex) fn join_sym_tkn(
        &mut self,
        curr_tkn: &LocatedTkn,
        adjacent: bool,
    ) -> Option<Tkn> {
        if !adjacent {
            return None;
        }
        match &curr_tkn.tkn {
            Tkn::Colon => {
                let tkn = match &self.gen_tkns.last()?.tkn {
                    Tkn::Colon => Tkn::ModPathTkn,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(tkn)
            }
            Tkn::Add => {
                let t = match self.gen_tkns.last()?.tkn {
                    Tkn::Add => Tkn::Inc,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(t)
            }
            Tkn::Sub => {
                let t = match self.gen_tkns.last()?.tkn {
                    Tkn::Sub => Tkn::Dec,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(t)
            }
            Tkn::LAngleBracket => {
                let t = match self.gen_tkns.last()?.tkn {
                    Tkn::LAngleBracket => Tkn::LShift,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(t)
            }
            Tkn::RAngleBracket => {
                let t = match self.gen_tkns.last()?.tkn {
                    Tkn::RAngleBracket => Tkn::RShift,
                    // `=>` (matchの条件/パターンの後に付ける矢印)
                    Tkn::Equal => Tkn::Arrow,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(t)
            }
            // `=` は直前が `=` なら `==`、直前が `!` なら `!=` として結合する。
            // 修正前は `!=` の判定が誤って `Tkn::Not` 側（2文字目が `!` の場合）
            // に書かれており、2文字目として `=` が来たときに直前の `Not` と
            // 結合できず、`!=` が `Not, Equal` という別々のトークンに
            // 分かれてしまっていた。
            Tkn::Equal => {
                let tkn = match &self.gen_tkns.last()?.tkn {
                    Tkn::Equal => Tkn::EqEq,
                    Tkn::Not => Tkn::NotEq,
                    Tkn::Add => Tkn::AddEq,
                    Tkn::Sub => Tkn::SubEq,
                    Tkn::Mul => Tkn::MulEq,
                    Tkn::Div => Tkn::DivEq,
                    Tkn::LAngleBracket => Tkn::LessThanEq,
                    Tkn::RAngleBracket => Tkn::GreaterThanEq,
                    _ => return None,
                };
                self.gen_tkns.pop();
                Some(tkn)
            }
            Tkn::Dot => {
                if matches!(self.gen_tkns.last()?.tkn, Tkn::Dot) {
                    self.gen_tkns.pop();
                    Some(Tkn::RangeTkn)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
