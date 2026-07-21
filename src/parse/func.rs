use super::{Parser, *};



/// 関数を定義するノードの生成
impl Parser {
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
                            is_public,
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
                t => {panic!("{:?}", t)},
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

        let lex::Tkn::Name(name) = self.next_tkn()? else { panic!("unexpect name") };

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
                    node::TyNode::RefTy(generics.0.clone())
                } else {
                    node::TyNode::Ty(name.clone())
                }
            }
        };
        Ok(ty)
    }
}

