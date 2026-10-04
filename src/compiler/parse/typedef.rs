mod def_fields;

use super::{Parser, *};
use std::collections::HashMap;

/// 共用体の追加オプションのフラグ
#[derive(Debug, Clone)]
pub(in crate::compiler::parse::typedef)
enum UnionFlags {
    Unsafe,
    Normal,
}

/// 構造体、共用体、列挙型を定義するノードの生成
impl Parser {
    /// `struct name { mem: ty, mem2: ty2 }` を解析する
    /// 呼び出し時は current_tkn() が KeyWordStruct
    pub(in crate::compiler::parse) 
    fn struct_node(&mut self) -> Result<node::Group1Node, err::ErrKind> {
        let lex::Tkn::Name(name) = self.next_tkn(&["name"])? else {
            return self.struct_name_is_not_found();
        };
        // 自身の構造体の名前を登録、`Self`をこれに入れ替える
        self.struct_self_name = Some(name.to_string());

        match self.next_tkn(&["{"])? {
            lex::Tkn::LBrace => {}
            t => {
                self.struct_lbrace_not_found(t)?;
            }
        }

        let fields = self.define_struct_fields()?;
        // 構造体の中身をすべて処理し終わったので、`None`にする
        self.struct_self_name = None;
        self.gen_flag = GenFlag::Group1;
        let r = node::StructDefine::new(name, fields.0, fields.1);
        Ok(r)
    }

    /// `union name { mem: ty, mem2: ty2 }` を解析する
    /// 呼び出し時は current_tkn() が KeyWordUnion
    pub(in crate::compiler::parse)
    fn union_node(&mut self) -> Result<node::Group1Node, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::KeyWordUnion {
            return self.union_keyword_not_found();
        }

        let union_flag = match self.next_tkn(&["name", "str litral"])? {
            lex::Tkn::Name(union_name) => {
                // 自身の構造体の名前を登録、`Self`をこれに入れ替える
                self.struct_self_name.insert(union_name);
                UnionFlags::Normal
            }
            lex::Tkn::Str(union_option) => {
                match union_option.as_str() {
                    "unsafe" => {
                        if let Some(union_name) = self.advance_tkn() {
                            // 自身の構造体の名前を登録、`Self`をこれに入れ替える
                            self.struct_self_name.insert(union_name);
                        } else {
                            // オプションの続きに共用体の名前が来なかった
                            return self.union_option_next_name_not_found();
                        }
                        UnionFlags::Unsafe
                    }
                    _ => return self.union_option_unregister(union_option),
                }
            }
            _ => return self.union_name_is_not_found(),
        };

        match self.next_tkn(&["{"])? {
            lex::Tkn::LBrace => {}
            t => {
                self.union_lbrace_not_found(t)?;
            }
        }

        let fields = self.define_union_fields(union_flag)?;
        // 構造体の中身をすべて処理し終わったので、`None`にする
        self.struct_self_name = None;
        self.gen_flag = GenFlag::Group1;
        let r = node::UnionDefine::new(name, fields.0, fields.1);
        Ok(r)
    }

    /// 構造体を初期化する式を生成
    pub(in crate::compiler::parse) 
    fn struct_init_node<const T: bool>(
        &mut self,
        name: &str,
    ) -> Result<node::Expr, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            self.struct_lbrace_not_found(self.current_tkn().clone())?;
        }
        // {を飛ばす
        let _ = self.next_tkn(&[])?;
        let mut fields = HashMap::<String, Box<node::Expr>>::new();

        loop {
            let name: String = match &self.current_tkn() {
                lex::Tkn::Name(name) => name.to_string(),
                lex::Tkn::RBrace => {
                    break;
                }
                t => {
                    return crate::syntax_err!(
                        self.build_err_span(),
                        err::SyntaxErrKind::UnexpectedTkn {
                            found: (*t).clone(),
                            expected: lex::Tkn::Name("struct init".to_string()),
                            context: lex::Tkn::KeyWordStruct
                        }
                    );
                }
            };
            // :じゃないとエラー
            if self.next_tkn(&[":"])? != lex::Tkn::Colon {
                self.struct_in_unexpect_tkn(lex::Tkn::Colon)?;
            }
            fields.insert(
                name,
                // 構造体を初期化する
                Box::new(self.expr_cmp(true)?),
            );

            match self.current_tkn() {
                lex::Tkn::Name(..) => {
                    continue;
                }
                lex::Tkn::RBrace => break,
                t => panic!("{:?}", t),
            }
        }
        Ok(node::Expr::InitStruct {
            is_self: T,
            name: name.to_string(),
            fields: fields.clone(),
        })
    }

    /// 共用体を初期化する式を生成
    /// 呼び出し時は current_tkn() が `LBrace` (`struct_init_node`と同じ)
    pub(in crate::compiler::parse)
    fn union_init_node<const T: bool>(
        &mut self,
        name: &str,
    ) -> Result<node::Expr, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            self.union_lbrace_not_found(self.current_tkn().clone())?;
        }
        // {を飛ばす
        let _ = self.next_tkn(&[])?;
        let mut fields = HashMap::<String, Box<node::Expr>>::new();

        loop {
            let name: String = match &self.current_tkn() {
                lex::Tkn::Name(name) => name.to_string(),
                lex::Tkn::RBrace => {
                    break;
                }
                t => {
                    return crate::syntax_err!(
                        self.build_err_span(),
                        err::SyntaxErrKind::UnexpectedTkn {
                            found: (*t).clone(),
                            expected: lex::Tkn::Name("union init".to_string()),
                            context: lex::Tkn::KeyWordUnion
                        }
                    );
                }
            };
            // :じゃないとエラー
            if self.next_tkn(&[":"])? != lex::Tkn::Colon {
                self.union_in_unexpect_tkn(lex::Tkn::Colon)?;
            }
            fields.insert(
                name,
                // 共用体のメンバーを初期化する
                Box::new(self.expr_cmp(true)?),
            );

            match self.current_tkn() {
                lex::Tkn::Name(..) => continue,
                lex::Tkn::RBrace => break,
                // 値の後ろに続けられるのは、次のメンバー名か`}`だけ
                _ => self.union_in_unexpect_tkn(lex::Tkn::RBrace)?,
            }
        }
        Ok(node::Expr::InitUnion {
            is_self: T,
            name: name.to_string(),
            fields,
        })
    }

    /// `enum name { mem, mem2 }` を解析する
    /// 呼び出し時は current_tkn() が `KeyWordEnum`
    pub(in crate::compiler::parse) 
    fn enum_node(&mut self) -> Result<node::Group1Node, err::ErrKind> {
        let lex::Tkn::Name(name) = self.next_tkn(&["name"])? else {
            panic!("列挙型の名前が必要です");
        };

        match self.next_tkn(&["{"])? {
            lex::Tkn::LBrace => {}
            t => panic!("{:?}", t),
        }

        let variants = self.define_enum_variants()?;
        Ok(node::EnumDefine::new(name, variants.0))
    }

    /// 列挙型のメンバ定義を解析する
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace` を指す
    fn define_enum_variants(
        &mut self,
    ) -> Result<(Vec<String>, Vec<node::Group1Node>), err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return Err(err::ErrKind::NotFoundTkn(Box::new(lex::Tkn::LBrace)));
        }

        let mut variants = Vec::<String>::new();
        let mut pub_flag = false;
        let mut methods = Vec::new();

        loop {
            match self.next_tkn(&["name", "pub", "}"])? {
                lex::Tkn::Name(name) => {
                    variants.push(name.to_string());

                    // 次のトークンを先読みし、`,`があるかどうかだけで
                    // 位置を進めるかを決める
                    // (`,`が無い場合はcurrent_tknを次のメンバーの
                    // `name`/`pub`のまま残し、ループの先頭の`next_tkn`で
                    // もう一度読めるようにする)
                    match self.next_tkn_ref(&["}", ",", "(", "name", "pub"])? {
                        lex::Tkn::RBrace => {
                            self.next_tkn(&[])?;
                            break;
                        }
                        lex::Tkn::Name(..) => {
                            continue;
                        }
                        // `func_node`が`(`を含めて解析するので、
                        // ここではcurrent_tknを`name`のままにしておく
                        lex::Tkn::LParen => {
                            methods.push(self.func_node(&name, pub_flag.clone())?);
                        }
                        // `,`を省略して改行だけで次のメンバーへ続く場合
                        lex::Tkn::KeyWordPub => {}
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

    fn ptr_ty_node(&mut self, ty: node::TyNode) -> Result<node::TyNode, err::ErrKind> {
        let range = if self.peek_tkn()? == lex::Tkn::LParen {
            // ポインタの範囲指定がある場合、`(start, end)`を読み込む
            Some((0usize, 0usize))
        } else {
            None
        };
        let is_const = self.is_const_ptr()?;
        self.next_tkn(&[])?;

        Ok(node::TyNode::Pointer {
            is_const,
            ty_name: Box::new(ty),
            range: range,
        })
    }

    #[inline(always)]
    fn is_const_ptr(&mut self) -> Result<bool, err::ErrKind> {
        let is_mut = match self.next_tkn_ref(&["const", "mut"])? {
            lex::Tkn::KeyWordMut => {
                self.advance_tkn();
                true
            }
            lex::Tkn::KeyWordConst => {
                self.advance_tkn();
                false
            }
            _ => false,
        };

        Ok(is_mut)
    }

    /// 型のノードを作成する
    ///
    /// ## self_name
    /// メゾット(構造体/列挙型に定義された関数)の中で型を解析する場合、
    /// その構造体/列挙型自身の名前を渡す。
    /// これにより、型として予約語`Self`が使われたとき、
    /// `node::TyNode::SelfTy(self_name)`へ解決できる。
    /// メゾットの外(トップレベルの関数など)では`None`を渡す。
    pub(in crate::compiler::parse) 
    fn define_ty_node(&mut self) -> Result<node::TyNode, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::Colon {
            return Err(err::ErrKind::UnexpectedToken);
        }
        self.ty_node_after_delim()
    }

    /// 型のノードを作成する。`define_ty_node`と違い、型の1つ手前の
    /// トークンが`:`である必要はなく、`current_tkn()`は型の直前の
    /// 区切り(`:`、ジェネリクスの型引数の`<`や`,`)を指していればよい。
    /// 終了時は、型の次のトークンを指す
    pub(in crate::compiler::parse) 
    fn ty_node_after_delim(&mut self) -> Result<node::TyNode, err::ErrKind> {
        match self.next_tkn(&["name", "[", "string", "Self"])?.clone() {
            // 予約語`Self`: 自身の構造体を指す型
            lex::Tkn::KeyWordSelf => {
                // `Self`の次のトークンへ進める
                self.next_tkn(&[])?;

                let self_name = self
                    .struct_self_name
                    .as_ref()
                    .expect("`Self`は構造体/列挙型のメソッド内でのみ使用できます");

                let ty = match self.current_tkn() {
                    lex::Tkn::Mul => {
                        // ポインタ化する場合はここで ty を組み立てる
                        let ty = node::TyNode::SelfTy(self_name.clone());
                        self.ptr_ty_node(ty)?
                    }
                    _ => node::TyNode::SelfTy(self_name.clone()),
                };

                Ok(ty)
            }
            lex::Tkn::Name(name) => {
                // 契約型かを調べる
                let base_ty = self.is_constract_ty(node::TyNode::Ty(name.clone()))?;
                let ty =
                    match self.next_tkn(&["<", "*", "["])? {
                        lex::Tkn::LAngleBracket => {
                            return crate::syntax_err!(
                                self.build_err_span(),
                                err::SyntaxErrKind::GenericsNotSupportedYet
                            );
                        }
                        // 境界付きポインタ
                        lex::Tkn::LBracket => {
                            self.next_tkn(&["["])?;
                            self.make_range_ptr_node(base_ty)?
                        }
                        // ポインタの型
                        // var: *.. の `*`
                        lex::Tkn::Mul => self.ptr_ty_node(base_ty)?,
                        _ => {
                            // ジェネリクスに定義ずみの型がある
                            if self.other_stk.iter().any(|(stk_name, info)| {
                                stk_name == &name && info == &StkInfo::Generics
                            }) {
                                node::TyNode::make_ref_ty(&name)
                            } else {
                                base_ty
                            }
                        }
                    };
                Ok(ty)
            }
            // スタック領域: `[ty]` / `[ty 4]`
            lex::Tkn::LBracket => self.define_mem_ty_node(false),
            // 静的領域: `static[ty]` / `static[ty 4]`
            lex::Tkn::KeyWordStatic => {
                if self.next_tkn(&["["])? != lex::Tkn::LBracket {
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
        let ty_name = self.next_tkn(&["name"])?.unwrap_name();

        let len = match self.next_tkn(&["number", "]"])? {
            lex::Tkn::Number(num) => {
                let len = num
                    .parse::<usize>()
                    .expect("配列の長さは数値である必要があります");

                if self.next_tkn(&["not `]`"])? != lex::Tkn::RBracket {
                    panic!("`]`が必要です");
                }
                len
            }
            lex::Tkn::RBracket => 1,
            t => panic!("unexpect token in mem ty: {:?}", t),
        };
        // `]`の次のトークンに進める
        // (呼び出し元が`=`などを current_tkn() で判定できるように)
        self.next_tkn(&[])?;

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
        let _ = lex.analy(&value.to_string()).unwrap();
        let mut parse = Parser::new();
        parse.parser(&lex.gen_tkns.clone()).unwrap().to_vec()
    }

    /// 構文解析がエラーになるかどうか
    fn build_is_err(value: &str) -> bool {
        let mut lex = lex::Lexer::new();
        let _ = lex.analy(&value.to_string()).unwrap();
        let mut parse = Parser::new();
        parse.parser(&lex.gen_tkns.clone()).is_err()
    }

    #[test]
    fn test_struct() {
        assert_eq!(
            &build("struct Name { name: ty name2: ty }"),
            &vec![node::StructDefine::new(
                "Name".to_string(),
                vec![
                    node::StructField::make_field("name", "ty"),
                    node::StructField::make_field("name2", "ty")
                ],
                Vec::new()
            )]
        );
    }

    #[test]
    fn struct_init() {
        let mut func = node::FuncDefine::new(
            "main",
            Vec::new(),
            node::TyNode::Ty("int".to_string()),
            false,
        );
        let map: HashMap<String, Box<node::Expr>> = [
            (
                "name".to_string(),
                Box::new(node::Expr::Number("1".to_string())),
            ),
            (
                "name2".to_string(),
                Box::new(node::Expr::Number("1".to_string())),
            ),
        ]
        .into_iter()
        .collect();
        let node::Group1Node::FuncDefine(ref mut f) = func else {
            panic!();
        };
        f.add(
            node::Group2Node::Expr(node::Expr::InitStruct {
                is_self: false,
                name: "Name".to_string(),
                fields: map,
            })
            .gen_group_info(1),
        );
        assert_eq!(
            &build("main(): int { Name { name: 1 name2: 1 } }"),
            &vec![func]
        );
    }

    #[test]
    fn struct_method() {
        let mut f = vec![node::FuncDefine::new(
            "new",
            Vec::new(),
            node::TyNode::Ty("ty".to_string()),
            false,
        )];
        let node::Group1Node::FuncDefine(func) = f.last_mut().unwrap() else {
            panic!();
        };
        func.self_module_name(&"Name".to_string());
        assert_eq!(
            &build("struct Name { a: int new(): ty {}}"),
            &vec![node::StructDefine::new(
                "Name".to_string(),
                vec![node::StructField::make_field("a", "int"),],
                f
            )]
        );
    }

    #[test]
    fn struct_method_self_ty() {
        // `Self`は、メゾットが定義されている構造体自身の名前へ
        // 解決される(第一引数/戻り値のどちらでも使用可能)
        let nodes = build(
            "struct Name { \
                a: int \
                new(): Self {} \
                func(self: Self, param: int): int {ret 0} \
            }",
        );
        let node::Group1Node::StructDefine(struct_def) = &nodes[0] else {
            panic!("構造体が定義されていません: {:?}", nodes);
        };

        let node::Group1Node::FuncDefine(new_func) = &struct_def.methods[0] else {
            panic!();
        };
        assert_eq!(new_func.name, "new");
        assert_eq!(new_func.ret_ty, node::TyNode::SelfTy("Name".to_string()));

        let node::Group1Node::FuncDefine(func) = &struct_def.methods[1] else {
            panic!();
        };
        assert_eq!(func.name, "func");
        assert_eq!(func.params[0].ty, node::TyNode::SelfTy("Name".to_string()));
        assert_eq!(func.params[0].name, "self");
        assert_eq!(func.params[1].name, "param");
        assert_eq!(func.params[1].ty, node::TyNode::Ty("int".to_string()));
    }

    #[test]
    fn test_union() {
        assert_eq!(
            &build("union Name { name: ty name2: ty }"),
            &vec![node::UnionDefine::new(
                "Name".to_string(),
                vec![
                    node::StructField::make_field("name", "ty"),
                    node::StructField::make_field("name2", "ty")
                ],
                Vec::new()
            )]
        );
    }

    #[test]
    fn union_method() {
        let mut f = vec![node::FuncDefine::new(
            "new",
            Vec::new(),
            node::TyNode::Ty("ty".to_string()),
            false,
        )];
        let node::Group1Node::FuncDefine(func) = f.last_mut().unwrap() else {
            panic!();
        };
        func.self_module_name(&"Name".to_string());
        assert_eq!(
            &build("union Name { a: int new(): ty {}}"),
            &vec![node::UnionDefine::new(
                "Name".to_string(),
                vec![node::StructField::make_field("a", "int"),],
                f
            )]
        );
    }

    #[test]
    fn union_method_self_ty() {
        // 共用体でも`Self`は、定義されている共用体自身の名前へ解決される
        let nodes = build(
            "union Name { \
                a: int \
                new(): Self {} \
                func(self: Self, param: int): int {ret 0} \
            }",
        );
        let node::Group1Node::UnionDefine(union_def) = &nodes[0] else {
            panic!("共用体が定義されていません: {:?}", nodes);
        };

        let node::Group1Node::FuncDefine(new_func) = &union_def.methods[0] else {
            panic!();
        };
        assert_eq!(new_func.name, "new");
        assert_eq!(new_func.ret_ty, node::TyNode::SelfTy("Name".to_string()));

        let node::Group1Node::FuncDefine(func) = &union_def.methods[1] else {
            panic!();
        };
        assert_eq!(func.name, "func");
        assert_eq!(func.params[0].ty, node::TyNode::SelfTy("Name".to_string()));
        assert_eq!(func.params[0].name, "self");
        assert_eq!(func.params[1].name, "param");
        assert_eq!(func.params[1].ty, node::TyNode::Ty("int".to_string()));
    }

    #[test]
    fn union_name_is_not_found() {
        assert!(build_is_err("union { a: int }"));
    }

    #[test]
    fn union_lbrace_not_found() {
        assert!(build_is_err("union Name a: int }"));
    }

    #[test]
    fn enum_node() {
        assert_eq!(
            &build("enum Name {A B}"),
            &vec![node::EnumDefine::new(
                "Name".to_string(),
                vec!["A".to_string(), "B".to_string()]
            )]
        );
    }
}