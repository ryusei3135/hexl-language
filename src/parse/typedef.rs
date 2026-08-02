use super::{Parser, *};
use std::collections::HashMap;

/// 構造体、列挙型を定義するノードの生成
impl Parser {
    /// `struct name { mem: ty, mem2: ty2 }` を解析する
    /// 呼び出し時は current_tkn() が KeyWordStruct
    pub(super) fn struct_node(&mut self) -> Result<node::Group1Node, err::ErrKind> {
        let lex::Tkn::Name(name) = self.next_tkn(vec!["name"])? else {
            panic!("構造体の名前が必要です");
        };

        match self.next_tkn(vec!["{"])? {
            lex::Tkn::LBrace => {},
            t => panic!("{:?}", t),
        }

        let fields = self.define_struct_fields(&name)?;
        Ok(node::StructDefine::new(name, fields.0, fields.1))
    }

    /// 構造体を初期化する式を生成
    pub(super) fn struct_init_node(
        &mut self,
        name: &String
    ) -> Result<node::Expr, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            panic!("typdef::strct_init_node {:?}", self.current_tkn());
        }

        // {を飛ばす
        let _ = self.next_tkn(vec![])?;

        let mut fields = HashMap::<String, Box<node::Expr>>::new();

        loop {
            let name: String = match &self.current_tkn() {
                lex::Tkn::Name(name) => {
                    name.to_string()
                }
                lex::Tkn::RBrace => {
                    break;
                }
                t => panic!("{:?}", t),
            };
            // :じゃないとエラー
            if !matches!(self.next_tkn(vec![":"])?, lex::Tkn::Colon) {
                panic!("{:?} {:?} name {:?}", self.current_tkn(), self.next_tkn_ref(vec![])?, name);    
            }
            fields.insert(
                name,
                // 構造体を初期化する
                Box::new(self.expr_cmp(true)?)
            );

            match self.current_tkn() {
                lex::Tkn::Comma => {
                    // ","を飛ばして次のフィールドへ
                    self.next_tkn(vec![])?;
                }
                lex::Tkn::RBrace => break,
                t => panic!("{:?}", t),
            }
        }
        Ok(
            node::Expr::InitStruct{
                name: name.to_string(),
                fields: fields.clone()
            }
        )
    }

    /// `enum name { mem, mem2 }` を解析する
    /// 呼び出し時は current_tkn() が `KeyWordEnum`
    pub(super) fn enum_node(&mut self) -> Result<node::Group1Node, err::ErrKind> {
        let lex::Tkn::Name(name) = self.next_tkn(vec!["name"])? else {
            panic!("列挙型の名前が必要です");
        };

        match self.next_tkn(vec!["{"])? {
            lex::Tkn::LBrace => {},
            t => panic!("{:?}", t),
        }

        let variants = self.define_enum_variants()?;
        Ok(node::EnumDefine::new(name, variants.0))
    }

    /// 構造体のメンバ定義を解析する関数
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace`
    fn define_struct_fields(
        &mut self,
        struct_name: &String,
    ) -> Result<(Vec<node::StructField>, Vec<node::Group1Node>), err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return Err(err::ErrKind::NotFoundTkn(lex::Tkn::LBrace));
        }

        let mut fields = Vec::<node::StructField>::new();
        let mut pub_flag = false;
        let mut methods = Vec::new();

        loop {
            match self.next_tkn(vec!["name", "pub", "}"])? {
                lex::Tkn::Name(name) => {
                    match self.next_tkn_ref(vec![":", "("])? {
                        lex::Tkn::Colon => {
                            self.next_tkn(vec![])?;
                            let ty = self.define_ty_node()?;

                            fields.push(node::StructField {
                                name: name.clone(),
                                ty,
                            });

                            match self.current_tkn() {
                                lex::Tkn::Comma => {}
                                lex::Tkn::RBrace => break,
                                t => {
                                    err::SyntaxErr::unexpect_tkn_after_keyword(
                                        self.build_err_span(),
                                        lex::Tkn::Colon,
                                        vec![",", "}"],
                                        t
                                    )?
                                }
                            }
                            continue;
                        }

                        lex::Tkn::LParen => {
                            let mut method = self.func_node(&name, pub_flag)?;
                            // モジュールの名前を登録
                            if let node::Group1Node::FuncDefine(ref mut func) = method {
                                func.self_module_name(&struct_name);
                            } else {
                                panic!();
                            }
                            methods.push(method);
                        }

                        _ => return Err(err::ErrKind::UnexpectedToken),
                    }
                    pub_flag = false;
                    self.next_tkn(vec![])?;
                }
                lex::Tkn::KeyWordPub => {
                    pub_flag = true;
                }
                lex::Tkn::RBrace => break,
                t => panic!("{:?}", t),
            }
        }

        Ok((fields, methods))
    }

    /// 列挙型のメンバ定義を解析する
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace` を指す
    fn define_enum_variants(
        &mut self
    ) -> Result<(Vec<String>, Vec<node::Group1Node>), err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return Err(err::ErrKind::NotFoundTkn(lex::Tkn::LBrace));
        }

        let mut variants = Vec::<String>::new();
        let mut pub_flag = false;
        let mut methods = Vec::new();

        loop {
            match self.next_tkn(vec!["name", "pub", "}"])? {
                lex::Tkn::Name(name) => {
                    variants.push(name.to_string());

                    match self.next_tkn(vec!["}", ",", "("])? {
                        lex::Tkn::RBrace => break,
                        lex::Tkn::Comma => {},
                        lex::Tkn::LParen => methods.push(self.func_node(&name, pub_flag.clone())?),
                        t => panic!("{:?}", t),
                    }
                    pub_flag = false;
                }
                lex::Tkn::KeyWordPub => {
                    pub_flag = true;
                }
                lex::Tkn::RBrace => break,
                t => panic!("{:?}", t),
            }
        }

        Ok((variants, methods))
    }

    /// 型のノードを作成する
    pub(super) fn define_ty_node(&mut self) -> Result<node::TyNode, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::Colon {
            return Err(err::ErrKind::UnexpectedToken);
        }

        match self.next_tkn(vec!["name", "[", "string"])? {
            lex::Tkn::Name(name) => {
                let ty = match self.next_tkn(vec!["<", "*"])? {
                    lex::Tkn::LAngleBracket => panic!(),
                    // ポインタの型
                    lex::Tkn::Mul => {
                        let is_const = 
                            if matches!(self.next_tkn_ref(vec!["const"])?,lex::Tkn::KeyWordConst) {
                                true
                            } else {
                                false
                            };
                        self.next_tkn(vec![])?;
                        
                        node::TyNode::Pointer {
                            is_const,
                            ty_name: Box::new(node::TyNode::Ty(name.clone()))
                        }
                    }
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
                if !matches!(self.next_tkn(vec!["["])?, lex::Tkn::LBracket) {
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
        let lex::Tkn::Name(ty_name) = self.next_tkn(vec!["name"])? else {
            panic!("スタック/静的領域の型名が必要です");
        };

        let len = match self.next_tkn(vec!["number", "]"])? {
            lex::Tkn::Number(num) => {
                let len = num
                    .parse::<usize>()
                    .expect("配列の長さは数値である必要があります");

                if !matches!(self.next_tkn(vec!["not `]`"])?, lex::Tkn::RBracket) {
                    panic!("`]`が必要です");
                }
                len
            }
            lex::Tkn::RBracket => 1,
            t => panic!("unexpect token in mem ty: {:?}", t),
        };
        // `]`の次のトークンに進める
        // (呼び出し元が`=`などを current_tkn() で判定できるように)
        self.next_tkn(vec![])?;

        if is_static {
            Ok(node::TyNode::Static { name: ty_name, len })
        } else {
            Ok(node::TyNode::Stack { name: ty_name, len })
        }
    }
}


#[cfg(test)]
mod ty_tests {
    use super::*;
    use std::collections::HashMap;

    fn build(value: &str) -> Vec<node::Group1Node> {
        let mut lex = lex::Lexer::new();
        let _ = lex
            .analy(&value.to_string())
            .unwrap();
        let mut parse = Parser::new();
        parse.parser(lex.gen_tkns.clone()).unwrap().clone()
    }

    #[test]
    fn test_struct() {
        assert_eq!(
            &build("struct Name { name: ty, name2: ty }"),
            &vec![
                node::StructDefine::new(
                    "Name".to_string(),
                    vec![
                        node::StructField::make_field("name", "ty"),
                        node::StructField::make_field("name2", "ty")
                    ],
                    Vec::new()
                )
            ]
        );
    }

    #[test]
    fn struct_init() {
        let mut func = 
            node::FuncDefine::new(
                "main".to_string(), 
                Vec::new(), 
                node::TyNode::Ty("int".to_string()),
                false
            );
        let map: HashMap<String, Box<node::Expr>> = [
            ("name".to_string(), Box::new(node::Expr::Number("1".to_string()))),
            ("name2".to_string(), Box::new(node::Expr::Number("1".to_string())))
        ].into_iter().collect();
        let node::Group1Node::FuncDefine(ref mut f) = func else {
            panic!();
        };
        f.add(node::Group2Node::Expr(node::Expr::InitStruct { name: "Name".to_string(), fields: map}));
        assert_eq!(
            &build("main(): int { Name { name: 1, name2: 1 } }"),
            &vec![
                func
            ]
        );
    }

    #[test]
    fn struct_method() {
        let mut f = 
            vec![node::FuncDefine::new(
                "new".to_string(),
                Vec::new(),
                node::TyNode::Ty("ty".to_string()),
                false
            )];
        let node::Group1Node::FuncDefine(func) = f.last_mut().unwrap() else {
            panic!();
        };
        func.self_module_name(&"Name".to_string());
        assert_eq!(
            &build("struct Name { a: int, new(): ty {}}"),
            &vec![
                node::StructDefine::new(
                    "Name".to_string(),
                    vec![
                        node::StructField::make_field("a", "int"),
                    ],
                    f
                )
            ]
        );
    }

    #[test]
    fn enum_node() {
        assert_eq!(
            &build("enum Name {A, B}"),
            &vec![
                node::EnumDefine::new(
                    "Name".to_string(), 
                    vec!["A".to_string(), "B".to_string()]
                )
            ]
        );
    }
}
