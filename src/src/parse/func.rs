use super::{Parser, *};



/// 関数を定義するノードの生成
impl Parser {
    /// この関数を呼び出すときは、次のトークンが`(`で無ければ
    /// いけない
    pub(super) fn func_node(
        &mut self,
        func_name: &String,
        is_public: bool,
    ) -> Result<node::Group1Node, err::ErrKind> {
        let arg = match self.next_tkn()? {
            lex::Tkn::LParen => {
                self.define_arg_node()?
            }
            lex::Tkn::LAngleBracket => {panic!()},
            _ => panic!(),
        };

        match self.current_tkn() {
            lex::Tkn::Colon => {
                let ret_ty = self.define_ty_node()?;

                if self.current_tkn() == &lex::Tkn::LBrace {
                    Ok(
                        node::FuncDefine::new(
                            func_name.clone(),
                            arg,
                            ret_ty,
                            is_public
                        )
                    )
                } else {
                    Err(err::ErrKind::NotFoundTkn(lex::Tkn::LBrace))
                }
            },
            lex::Tkn::LBrace => {
                panic!();
            },
            t => panic!("{:?}", t),
        }
    }

    /// 引数の定義をするノードを作成する
    /// この関数では、型のあとまでトークンを進めているので、呼び出し元では
    /// current_tknでトークンを判定する
    fn define_arg_node(&mut self) -> Result<Vec<node::ArgsNode>, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LParen {
            return Err(err::ErrKind::NotFoundTkn(lex::Tkn::LParen));
        }

        let mut args_params = Vec::<node::ArgsNode>::new();
        // 引数の定義を作成可能か
        let mut can_create_param = true;

        loop {
            if !can_create_param {
                panic!();
            }
            match self.next_tkn()? {
                lex::Tkn::Name(name) => {
                    if !can_create_param {
                        panic!();
                    }
                    self.next_tkn()?;
                    let ty = self.define_ty_node()?;
                    args_params.push(node::ArgsNode {
                        name: name.clone(),
                        ty,
                    });
                    can_create_param = false;
                }
                lex::Tkn::RParen => {
                    self.next_tkn()?;
                    break;
                },
                t => {panic!("{:?} {:?}", t, self.next_tkn_ref()?)},
            }

            match self.current_tkn() {
                lex::Tkn::RParen => {
                    self.next_tkn()?;
                    break;
                }
                lex::Tkn::Comma => {
                    if !can_create_param {
                        can_create_param = true;
                    } else {
                        panic!();
                    } 
                }
                _ => panic!(),
            }
        }

        Ok(args_params)
    }

    pub(super) fn define_ty_node(&mut self) -> Result<node::TyNode, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::Colon {
            return Err(err::ErrKind::UnexpectedToken);
        }

        match self.next_tkn()? {
            lex::Tkn::Name(name) => {
                let ty = match self.next_tkn()? {
                    lex::Tkn::LAngleBracket => panic!(),
                    _ => {
                        // ジェネリクスに定義ずみの型がある
                        if let Some(generics) = self.other_stk
                            .iter()
                            .find(
                                |v| v.0 == name && &v.1 == &StkInfo::Generics
                            )
                        {
                            node::TyNode::make_ref_ty(&generics.0)
                        } else {
                            node::TyNode::Ty(name.clone())
                        }
                    }
                };
                Ok(ty)
            }
            // スタック領域: `[ty]` / `[ty 4]`
            lex::Tkn::LBracket => {
                self.define_mem_ty_node(false)
            }
            // 静的領域: `""[ty]` / `""[ty 4]`
            lex::Tkn::Str(ref s) if s.is_empty() => {
                if self.next_tkn()? != lex::Tkn::LBracket {
                    panic!("静的領域の定義には`[`が必要です");
                }
                self.define_mem_ty_node(true)
            }
            t => panic!("unexpect ty token: {:?}", t),
        }
    }

    /// スタック/静的領域に確保する変数の型を解析する
    /// 呼び出し時、`current_tkn()`は`LBracket`
    /// - `[ty]`   -> len == 1
    /// - `[ty N]` -> len == N
    ///
    /// 呼び出し終了時は、`]`の次のトークンを指す
    fn define_mem_ty_node(&mut self, is_static: bool) -> Result<node::TyNode, err::ErrKind> {
        let lex::Tkn::Name(ty_name) = self.next_tkn()? else {
            panic!("スタック/静的領域の型名が必要です");
        };

        let len = match self.next_tkn()? {
            lex::Tkn::Number(num) => {
                let len = num
                    .parse::<usize>()
                    .expect("配列の長さは数値である必要があります");

                if self.next_tkn()? != lex::Tkn::RBracket {
                    panic!("`]`が必要です");
                }
                len
            }
            lex::Tkn::RBracket => 1,
            t => panic!("unexpect token in mem ty: {:?}", t),
        };
        // `]`の次のトークンに進める
        // (呼び出し元が`=`などを current_tkn() で判定できるように)
        self.next_tkn()?;

        if is_static {
            Ok(node::TyNode::Static { name: ty_name, len })
        } else {
            Ok(node::TyNode::Stack { name: ty_name, len })
        }
    }
}
