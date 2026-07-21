use super::*;


impl Parser {
    pub(super) fn make_preproc(
        &mut self,
        proc_name: &String
    ) -> Result<node::Group2Node, err::ErrKind> {
        let result = match proc_name.as_str() {
            /*"define" => {},
            "undef" => {},*/
            "include" => {
                node::Group2Node::Include(self.build_mod_path()?)
            },
            /*"if" => {},
            "ifdef" => {},
            "ifndef" => {},
            "else" => {},
            "elif" => {},
            "endif" => {},
            "error" => {},*/
            "line" => {
                let curr_line = 
                    self.current_line()
                        .clone()
                        .to_string();
                node::Group2Node::Line(curr_line)
            }
            "asm" => self.build_asm_ast()?,
            _ => panic!(),
        };

        Ok(result)
    }


    fn build_mod_path(&mut self) -> Result<node::ModPath, err::ErrKind> {
        enum PathTkn {
            Name,
            PathTkn,
        }
        let mut flag: Option<PathTkn> = None;
        let mut mod_path = node::ModPath::new();
        loop {
            match self.next_tkn_ref()? {
                lex::Tkn::Name(name) => {
                    // 前回のトークンの種類が、無いまたは、"::"の場合だけ実行
                    if flag
                        .as_ref()
                        .is_none_or(|v| matches!(v, PathTkn::PathTkn))
                    {
                        mod_path.add_path(&name);
                        flag = Some(PathTkn::Name);
                    } else {
                        // pathの終了
                        break;
                    }
                }
                lex::Tkn::ModPathTkn => {
                    flag = Some(PathTkn::PathTkn);
                }
                _ => {
                    if flag
                        .as_ref()
                        .is_some_and(|v| matches!(v, PathTkn::Name))
                    {
                        break;
                    } else {
                        crate::preproc_err!(self, ExpectedPathSegment);
                    }
                }
            }
            self.next_tkn().unwrap();
        }
        Ok(mod_path)
    }

    /// ## 戻り値
    /// - Ok inlineアセンブラの名前
    fn build_asm_ast(&mut self) -> Result<node::Group2Node, err::ErrKind> {
        // #asm(...)なので、(以外が来たらエラー
        if self.next_tkn()? != lex::Tkn::LParen {
            crate::preproc_err!(self, ExpectedLParenAfterAsm);
        }

        let asm_name = if let lex::Tkn::Name(asm_name) = self.next_tkn()? {
            asm_name
        } else {
            crate::preproc_err!(self, NotFoundAsmName);
        };

        // #asm(...)なので、(以外が来たらエラー
        if self.next_tkn()? != lex::Tkn::RParen {
            crate::preproc_err!(self, ExpectedRParenAfterAsm);
        }
        let nodes = self.gen_asm_preproc()?;
        Ok(node::Group2Node::CompleSyntax((asm_name, nodes)))
    }

    /// アセンブリ言語のプロプロセッサの
    /// 中身(アセンブリ言語本体)を生成する関数
    fn gen_asm_preproc(&mut self) -> Result<Vec<String>, err::ErrKind> {
        let mut nodes = Vec::<String>::new();

        if self.next_tkn().is_ok_and(|v| &v == &lex::Tkn::LBrace) {
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
            return Ok(nodes);
        }

        Err(err::ErrKind::UnexpectedToken)
    }
}
