use super::*;


/// group1は、関数や構造体など
/// group2は変数の定義や条件分岐など
#[derive(Clone, Debug, PartialEq)]
pub(super) enum GenFlag {
    Group1,
    Group2,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ExpectTknKind {
    Expected(lex::Tkn),
    Free,
}


#[derive(Clone, Debug, PartialEq)]
pub(super) enum StkInfo {
    Generics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parser {
    gen_nodes: Vec<node::Group1Node>,
    tkns: Option<Vec<lex::LocatedTkn>>,
    idx: usize,
    pub(super) gen_flag: GenFlag,
    pub(super) other_stk: Vec<(String, StkInfo)>, // 処理中の一時データを保存
}

impl Parser {
    pub fn new() -> Self {
        Self {
            gen_nodes: Vec::new(),
            tkns: None,
            idx: 0,
            gen_flag: GenFlag::Group1,
            other_stk: Vec::new(),
        }
    }

    pub fn parser(
        &mut self,
        tkns: Vec<lex::LocatedTkn>,
        line: &usize
    ) -> Result<&Vec<node::Group1Node>, err::Errs> {
        self.tkns = Some(tkns);

        while self.idx < self.tkns.as_ref().unwrap().len() {
            match self.gen_flag {
                GenFlag::Group1 => {
                    match self.current_tkn().clone() {
                        lex::Tkn::Name(name) => {
                            let node = self.func_node(&name)
                                .map_err(|v| v.gen(&line, self.tkn_chr_pos()))?;
                            self.gen_nodes.push(node);

                            self.gen_flag = GenFlag::Group2;
                        }
                        t => panic!("{:?}", t),
                    };
                },
                GenFlag::Group2 => {
                    let node = match self.current_tkn().clone() {
                        lex::Tkn::KeyWord_Ret => {
                            node::StmtNode::Return(
                                self.expr_add()
                                .map_err(|v| v.gen(&line, self.tkn_chr_pos()))?
                            ).wrap()
                        }
                        t => panic!("{:?}", t),
                    };
                    if let Some(now_func) = self.gen_nodes.last_mut() {
                        match now_func {
                            node::Group1Node::FuncDefine(func) => {
                                func.add(node);
                            }
                            _ => panic!(),
                        }
                    } else {
                        panic!();
                    }
                },
            }
        }
        Ok(&self.gen_nodes)
    }

    pub(super) fn next_tkn(&mut self) -> Result<lex::Tkn, err::ErrKind> {
        self.idx += 1;
        if let Some(value) = self.tkns.as_deref().unwrap().get(self.idx) {
            Ok(value.tkn.clone())
        } else {
            Err(err::ErrKind::EndTkn)
        }
    }

    pub(super) fn expect_next_tkn(&mut self, expect_tkn: lex::Tkn) -> Result<bool, err::ErrKind> {
        self.idx += 1;
        if let Some(value) = self.tkns.as_ref().unwrap().get(self.idx) {
            let result = if value.tkn == expect_tkn {
                true
            } else {
                false
            };
            Ok(result)
        } else {
            Err(err::ErrKind::EndTkn)
        }
    }

    #[inline(always)]
    pub(super) fn current_tkn(&self) -> &lex::Tkn {
        &self.tkns.as_ref().unwrap()[self.idx].tkn
    }

    #[inline(always)]
    pub(super) fn tkn_chr_pos(&self) -> &usize {
        &self.tkns.as_ref().unwrap()[self.idx].pos
    }
}
