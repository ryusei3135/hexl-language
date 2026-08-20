use super::{Parser, *};

/// 関数を定義するノードの生成
impl Parser {
    /// この関数を呼び出すときは、次のトークンが`(`で無ければ
    /// いけない
    ///
    /// ## self_name
    /// 構造体/列挙型に定義されたメゾットを解析している場合、その
    /// 構造体/列挙型自身の名前を渡す。引数や戻り値の型に予約語
    /// `Self`が使われたとき、この名前へ解決するために使われる。
    /// トップレベルの関数を解析している場合は`None`を渡す。
    pub(super) fn func_node(
        &mut self,
        func_name: &String,
        is_public: bool,
    ) -> Result<node::Group1Node, err::ErrKind> {
        let arg = match self.next_tkn(vec!["(", "<"])? {
            lex::Tkn::LParen => self.define_arg_node()?,
            lex::Tkn::LAngleBracket => {
                panic!()
            }
            _ => panic!(),
        };

        match self.current_tkn() {
            lex::Tkn::Colon => {
                let ret_ty = self.define_ty_node()?;

                if self.current_tkn() == &lex::Tkn::LBrace {
                    Ok(node::FuncDefine::new(
                        func_name.clone(),
                        arg,
                        ret_ty,
                        is_public,
                    ))
                } else {
                    Err(err::ErrKind::NotFoundTkn(lex::Tkn::LBrace))
                }
            }
            // 戻り値の型が指定されていない場合、組み込みの`int`型を
            // デフォルトの戻り値の型として扱う
            lex::Tkn::LBrace => Ok(node::FuncDefine::new(
                func_name.clone(),
                arg,
                node::TyNode::Ty("int".to_string()),
                is_public,
            )),
            t => panic!("{:?}", t),
        }
    }

    /// 引数の定義をするノードを作成する
    /// この関数では、型のあとまでトークンを進めているので、呼び出し元では
    /// current_tknでトークンを判定する
    ///
    /// ## Panic
    /// - 初回以外で、`,`を挟まずに次の引数の定義なら
    /// - `,`の次に`)`が来た場合panic
    /// - `,`の次に`,`が来たら
    ///
    /// ## self_name
    /// メゾットの引数を解析している場合、そのメゾットが定義されている
    /// 構造体/列挙型自身の名前を渡す。引数の型に予約語`Self`が使われた
    /// 場合、`node::TyNode::SelfTy(self_name)`へ解決するために使われる
    /// (実際に`Self`が第一引数以外に使われていないかのチェックは、
    /// IRへの変換時に行う)
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
            match self.next_tkn(vec!["name", ")"])? {
                lex::Tkn::Name(name) => {
                    if !can_create_param {
                        panic!();
                    }
                    self.next_tkn(vec![])?;
                    let ty = self.define_ty_node()?;
                    args_params.push(node::ArgsNode {
                        name: name.clone(),
                        ty,
                    });
                    can_create_param = false;
                }
                // これが来た場合引数の定義が終了
                lex::Tkn::RParen => {
                    self.next_tkn(vec![])?;
                    break;
                }
                t => {
                    panic!("{:?} {:?}", t, self.next_tkn_ref(vec![])?)
                }
            }

            match self.current_tkn() {
                lex::Tkn::RParen => {
                    self.next_tkn(vec![])?;
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
}
