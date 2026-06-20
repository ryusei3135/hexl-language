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
    scope_counter: usize,
    pub(super) gen_flag: GenFlag,
    pub(super) other_stk: Vec<(String, StkInfo)>, // 処理中の一時データを保存
}

impl Parser {
    pub fn new() -> Self {
        Self {
            gen_nodes: Vec::new(),
            tkns: None,
            idx: 0,
            scope_counter: 0,
            gen_flag: GenFlag::Group1,
            other_stk: Vec::new(),
        }
    }

    pub fn parser(
        &mut self,
        tkns: Vec<lex::LocatedTkn>,
    ) -> Result<&Vec<node::Group1Node>, err::Errs> {
        self.tkns = Some(tkns);

        loop {
            match self.gen_flag {
                GenFlag::Group1 => {
                    match self.current_tkn().clone() {
                        lex::Tkn::Name(name) => {
                            let node = self.func_node(&name)
                                .map_err(|v| v.gen(self.current_line(), self.tkn_chr_pos()))?;
                            self.gen_nodes.push(node);

                            self.gen_flag = GenFlag::Group2;
                            self.scope_counter += 1;
                        }
                        t => panic!("{:?}", t),
                    };
                },
                GenFlag::Group2 => {
                    let node = match self.current_tkn().clone() {
                        lex::Tkn::CompleSyn => {
                            self.comple_syntax().map_err(|v| v.gen(self.current_line(), self.tkn_chr_pos()))?
                        }
                        lex::Tkn::Name(name) => {
                            node::Group2Node::Expr(
                                self.expr_define_var(name)
                                    .map_err(|v| v.gen(self.current_line(), self.tkn_chr_pos()))?
                            )
                        }
                        lex::Tkn::KeyWordRet => {
                            node::StmtNode::Return(
                                self.expr_add()
                                    .map_err(|v| v.gen(self.current_line(), self.tkn_chr_pos()))?
                            ).wrap()
                        }
                        lex::Tkn::RBrace => {
                            self.scope_counter -= 1;

                            if self.next_tkn().is_err() {
                                return Ok(&self.gen_nodes);
                            }

                            if self.scope_counter == 0 {
                                self.gen_flag = GenFlag::Group1;
                            }

                            continue;
                        }
                        t => panic!("{:?}", t),
                    };

                    match self.gen_nodes.last_mut().unwrap() {
                        node::Group1Node::FuncDefine(func) => {
                            func.add(node);
                        }
                    }
                    continue;
                },
            }
            if self.next_tkn().is_err() && self.scope_counter == 0 {
                return Ok(&self.gen_nodes);
            } 
        }
        Ok(&self.gen_nodes)
    }

    fn comple_syntax(&mut self) -> Result<node::Group2Node, err::ErrKind> {
        let lex::Tkn::Name(name) = self.next_tkn()? else {
            panic!();
        };

        let mut nodes = Vec::<String>::new();

        if let Ok(lex::Tkn::LBrace) = self.next_tkn() {
            let _ = self.next_tkn()?;
            loop {
                match self.current_tkn() {
                    lex::Tkn::Str(value) => {
                        nodes.push(value.clone());
                    }
                    lex::Tkn::RBrace => {
                        self.next_tkn()?;
                        break;
                    }
                    t => {
                        panic!("{:?}", t);
                    }
                }
                self.next_tkn()?;
            }
            return Ok(node::Group2Node::CompleSyntax((name, nodes)));
        }

        Err(err::ErrKind::UnexpectedToken)
    }

    pub(super) fn next_tkn(&mut self) -> Result<lex::Tkn, err::ErrKind> {
        self.idx += 1;
        if let Some(value) = self.tkns.as_ref().unwrap().get(self.idx) {
            Ok(value.tkn.clone())
        } else {
            Err(err::ErrKind::EndTkn)
        }
    }

    pub(super) fn next_tkn_ref(&self) -> Result<&lex::Tkn, err::ErrKind> {
        if let Some(value) = self.tkns.as_ref().unwrap().get(self.idx + 1) {
            Ok(&value.tkn)
        } else {
            Err(err::ErrKind::EndTkn)
        }
    }

    #[inline(always)]
    pub(super) fn current_tkn(&self) -> &lex::Tkn {
        &self.tkns.as_ref().unwrap()[self.idx].tkn
    }

    #[inline(always)]
    fn current_line(&self) -> &usize {
        &self.tkns.as_ref().unwrap()[self.idx].line
    }

    #[inline(always)]
    pub(super) fn tkn_chr_pos(&self) -> &usize {
        &self.tkns.as_ref().unwrap()[self.idx - 1].pos
    }
}
