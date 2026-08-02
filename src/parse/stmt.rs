//! group1は、関数や構造体など
//! group2は変数の定義や条件分岐など

use super::*;


#[derive(Clone, Debug, PartialEq)]
pub(super) enum GenFlag {
    Group1,
    Group2,
}


#[derive(Clone, Debug, PartialEq)]
pub(super) enum StkInfo {
    Generics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parser {
    gen_nodes: Vec<node::Group1Node>,
    pub(super) tkns: Option<Vec<lex::LocatedTkn>>,
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
    ) -> Result<&Vec<node::Group1Node>, err::ErrKind> {
        // 関数の中身などを作成する
        // P は、この関数が公開されるかどうかのbool
        fn build_func<const P: bool>(
            this: &mut Parser,
            func_name: &String
        ) -> Result<(), err::ErrKind> {
            let node = this.func_node(&func_name, P)?;
            this.gen_nodes.push(node);

            this.gen_flag = GenFlag::Group2;
            this.scope_counter += 1;
            Ok(())
        }

        self.tkns = Some(tkns);

        loop {
            match &self.gen_flag {
                GenFlag::Group1 => {
                    match self.current_tkn().clone() {
                        lex::Tkn::KeyWordPub => {
                            // この関数などは、公開する
                            match self.next_tkn(vec!["name", ".."])? {
                                lex::Tkn::Name(name) => {
                                    build_func::<true>(&mut *self, &name)
                                }
                                unexpect_tkn => {
                                    // 期待したトークンじゃないので、エラー
                                    err::SyntaxErr::unexpect_tkn_after_keyword(
                                        self.build_err_span(),
                                        lex::Tkn::KeyWordPub,
                                        vec!["struct", "enum", "name"],
                                        &unexpect_tkn,
                                    )
                                }
                            }?;
                        }
                        lex::Tkn::Name(name) => {
                            build_func::<false>(&mut *self, &name)?;
                        }
                        lex::Tkn::CompleSyn => {
                            let node = self.comple_syntax()?;
                            self.gen_nodes.push(node.change_group1());
                        }
                        lex::Tkn::KeyWordStruct => {
                            let node = self.struct_node()?;
                            self.gen_nodes.push(node);
                        }
                        lex::Tkn::KeyWordEnum => {
                            let node = self.enum_node()?;
                            self.gen_nodes.push(node);
                        }
                        t => panic!("{:?}", t),
                    };
                },
                GenFlag::Group2 => {
                    let node = self.one_line_node()?;

                    match self.gen_nodes.last_mut().unwrap() {
                        node::Group1Node::FuncDefine(func) => {
                            func.add(node);
                        }
                        t => panic!("KK {:?}", t),
                    }

                    if self.current_tkn() == &lex::Tkn::RBrace {
                        self.scope_counter -= 1;
                        if self.next_tkn(vec![]).is_err() {
                            return Ok(&self.gen_nodes);
                        }
                        
                        self.gen_flag = GenFlag::Group1;
                    }
                    continue;
                },
            }
            if self.next_tkn(vec![]).is_err() && self.scope_counter == 0 {
                return Ok(&self.gen_nodes);
            } 
        }
        //Ok(&self.gen_nodes)
    }

    pub(super) fn one_line_node(&mut self) -> Result<node::Group2Node, err::ErrKind> {
        let node = match self.current_tkn().clone() {
            lex::Tkn::CompleSyn => {
                self.comple_syntax()?
            }
            lex::Tkn::Name(name) => {
                node::Group2Node::Expr(self.build_scope_node(&name)?)
            }
            // ポインタにアクセスするノードの作成
            lex::Tkn::LBracket => {
                if let lex::Tkn::Name(name) = self.next_tkn(vec!["name"])?.clone() {
                    let mut ptr_connect = self.expr_define_var(name.to_string())?;
                    ptr_connect.get_assign_node().name = name.to_string();
                    let dst = ptr_connect.get_assign_node().clone().dst;
                    ptr_connect.get_assign_node().dst = Box::new(node::Expr::ConnectAddr(dst));
                    ptr_connect.wrap_group2()
                } else {
                    panic!();
                }
            }
            lex::Tkn::KeyWordRet => {
                node::StmtNode::Return(
                    self.expr_add(true)?
                ).wrap()
            }
            lex::Tkn::KeyWordLoop => {
                self.make_loop_node()?
            }
            // 条件分岐
            lex::Tkn::KeyWordCond => {
                let n = self.expr_match()?;
                node::Group2Node::Expr(n)
            }
            lex::Tkn::RBrace => {
                self.scope_counter -= 1;

                if self.scope_counter == 0 {
                    self.gen_flag = GenFlag::Group1;
                }
                panic!();
            }
            t => {
                panic!("parse stmt {:?}  {:?}", t, self.next_tkn_ref(vec![]));
            }
        };
        Ok(node)
    }

    /// 反復処理のノードを作成する関数
    fn make_loop_node(&mut self) -> Result<node::Group2Node, err::ErrKind> {
        // 反復処理の条件式
        let pattern = match self
            .next_tkn_ref(vec!["{", ".."])?
        {
            // "{"の場合は条件無し
            lex::Tkn::LBrace => {
                None
            }
            // 条件式あり
            _ => {
                // 条件に構造体の初期化を使うことはできない
                Some(Box::new(self.expr_cmp(false)?))
            }
        };
        // "{"をスキップ
        if self.current_tkn() != &lex::Tkn::LBrace {
            panic!();
        }
        self.next_tkn(vec!["{"])?;

        let body = self.gen_block_node()?;
        self.next_tkn(vec!["expr"])?;

        let node = node::Group2Node::Expr(
            node::Expr::Loop {
                pattern,
                body
            }
        );
        Ok(node)
    }

    /// 同じスコープ内のノードを生成
    pub(super) fn gen_block_node(&mut self) -> Result<Vec<node::Group2Node>, err::ErrKind> {
        let mut block = Vec::<node::Group2Node>::new();

        loop {
            let node = self.one_line_node()?;
            block.push(node);

            if self.current_tkn() == &lex::Tkn::RBrace {
                break;
            }
        }
        Ok(block)
    }

    pub(super) fn comple_syntax(&mut self) -> Result<node::Group2Node, err::ErrKind> {
        let lex::Tkn::Name(name) = self
            .next_tkn(vec![])? else
        {
            panic!();
        };
        self.make_preproc(&name)
    }

    /// 現在の位置から1つ先のトークンを取得する。
    /// `next_tkn_ref`と違い、それ以上トークンが無い場合は
    /// エラーではなく`None`を返す。
    ///
    /// inlineアセンブラの`${...}`内の式は、文の途中ではなく
    /// それだけで閉じた短いトークン列として解析されるため、
    /// 最後まで解析した後に続くトークンが存在しないことがある。
    /// そのため通常の文の解析(常に後続のトークンがある前提)
    /// とは違い、EOFをエラーにしない先読みが必要になる。
    pub(super) fn peek_tkn(&self) -> Option<lex::Tkn> {
        self.tkns
            .as_ref()
            .unwrap()
            .get(self.idx + 1)
            .map(|v| v.tkn.clone())
    }

    /// 次のトークンが存在する場合だけ位置を1つ進める。
    /// 存在しない場合は位置を変えずに`None`を返す
    pub(super) fn advance_tkn(&mut self) -> Option<lex::Tkn> {
        if let Some(next) = self.tkns.as_ref().unwrap().get(self.idx + 1) {
            self.idx += 1;
            Some(next.tkn.clone())
        } else {
            None
        }
    }

    /// なにのトークンが期待されていたかは呼び出し元で決める
    pub(super) fn next_tkn(
        &mut self,
        expected: Vec<&'static str>,
    ) -> Result<lex::Tkn, err::ErrKind> {
        self.idx += 1;
        if let Some(value) = self.tkns.as_ref().unwrap().get(self.idx) {
            Ok(value.tkn.clone())
        } else {
            err::SyntaxErr::tkn_is_eof(self.build_err_span(), expected)
        }
    }

    /// なにのトークンが期待されていたかは呼び出し元で決める
    pub(super) fn next_tkn_ref(
        &self,
        expected: Vec<&'static str>,
    ) -> Result<lex::Tkn, err::ErrKind> {
        if let Some(value) = self.tkns.as_ref().unwrap().get(self.idx + 1) {
            Ok(value.tkn.clone())
        } else {
            err::SyntaxErr::tkn_is_eof(self.build_err_span(), expected)
        }
    }

    /// エラーが発生したときのどの行の何文字目がエラーかを
    /// 確認する構造体を作成する
    pub fn build_err_span(&self) -> err::Span {
        err::Span::new(
            self.current_line(self.idx - 1),
            self.tkn_chr_pos()
        )
    }

    #[inline(always)]
    pub(super) fn current_tkn(&self) -> &lex::Tkn {
        &self.tkns.as_ref().unwrap()[self.idx].tkn
    }

    #[inline(always)]
    pub(super) fn current_line(&self, idx: usize) -> &usize {
        &self.tkns.as_ref().unwrap()[idx].line
    }

    #[inline(always)]
    pub(super) fn tkn_chr_pos(&self) -> &usize {
        &self.tkns.as_ref().unwrap()[self.idx - 1].pos
    }
}
