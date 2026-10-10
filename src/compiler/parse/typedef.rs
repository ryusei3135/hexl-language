mod def_fields;
mod variant_field;
mod generic;

use super::{Parser, *};
use std::collections::HashMap;

/// 構造体、バリアント型(共用体と列挙型を統合したもの)を定義するノードの生成
impl Parser {
    /// `struct name { mem: ty, mem2: ty2 }` を解析する
    /// 呼び出し時は current_tkn() が KeyWordStruct
    pub(in crate::compiler::parse) 
    fn struct_node(&mut self) -> Result<node::Group1Node, err::ErrKind> {
        let lex::Tkn::Name(name) = self.next_tkn(&["name"])? else {
            return crate::err_at!(self.struct_name_is_not_found());
        };
        // 自身の構造体の名前を登録、`Self`をこれに入れ替える
        self.struct_self_name = Some(name.to_string());

        match self.next_tkn(&["{"])? {
            lex::Tkn::LBrace => {}
            t => {
                crate::err_at!(self.struct_lbrace_not_found(t))?;
            }
        }

        let fields = self.define_struct_fields()?;
        // 構造体の中身をすべて処理し終わったので、`None`にする
        self.struct_self_name = None;
        self.gen_flag = GenFlag::Group1;
        let r = node::StructDefine::new(name, fields.0, fields.1);
        Ok(r)
    }

    /// バリアント型(旧`union`と旧`enum`を統合したもの)を解析する
    /// ```text
    /// variant Name { A B }                                  // 列挙型相当
    /// variant Name { Mem  Mem1(int)  func(self: Self): int {..} }
    /// variant "unsafe" Name { mem: int  mem2: i64 }        // タグなし(旧unsafe union)
    /// ```
    /// 呼び出し時は current_tkn() が KeyWordVariant
    pub(in crate::compiler::parse)
    fn variant_node(&mut self) -> Result<node::Group1Node, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::KeyWordVariant {
            return crate::err_at!(self.variant_keyword_not_found());
        }

        let (name, variant_mode) = match self.next_tkn(&["name", "str litral"])? {
            lex::Tkn::Name(variant_name) => (variant_name, node::VariantMode::Normal),
            lex::Tkn::Str(variant_option) => match variant_option.as_str() {
                "unsafe" => match self.next_tkn(&["name"])? {
                    lex::Tkn::Name(variant_name) => (variant_name, node::VariantMode::Unsafe),
                    // オプションの続きに共用体の名前が来なかった
                    _ => return crate::err_at!(self.variant_option_next_name_not_found()),
                },
                _ => return crate::err_at!(self.variant_option_unregister(variant_option)),
            },
            _ => return crate::err_at!(self.variant_name_is_not_found()),
        };
        // 自身の共用体の名前を登録、`Self`をこれに入れ替える
        self.struct_self_name = Some(name.clone());

        match self.next_tkn(&["{"])? {
            lex::Tkn::LBrace => {}
            lex::Tkn::LAngleBracket => {
                //
            }
            t => {
                crate::err_at!(self.variant_lbrace_not_found(t))?;
            }
        }

        let (fields, methods) = self.define_variant_fields(variant_mode)?;
        // 共用体の中身をすべて処理し終わったので、`None`にする
        self.struct_self_name = None;
        self.gen_flag = GenFlag::Group1;
        Ok(node::VariantDefine::new(name, fields, methods))
    }

    /// 構造体を初期化する式を生成
    pub(in crate::compiler::parse) 
    fn struct_init_node<const T: bool>(
        &mut self,
        name: &str,
    ) -> Result<node::Expr, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            crate::err_at!(self.struct_lbrace_not_found(self.current_tkn().clone()))?;
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
                    return crate::err_at!(self.struct_init_name_not_found((*t).clone()));
                }
            };
            // :じゃないとエラー
            if self.next_tkn(&[":"])? != lex::Tkn::Colon {
                crate::err_at!(self.struct_in_unexpect_tkn(lex::Tkn::Colon))?;
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

    /// バリアント型を初期化する式を生成
    /// ```text
    /// Variant::Name(expr)   // データ付きメンバー -> `Expr::InitVariant`
    /// Variant::Name         // データなしメンバー -> `Expr::EnumVariant`
    /// ```
    /// 呼び出し時は current_tkn() が `Variant::`の次の`Name`(メンバー名)
    /// 終了時は、式の最後のトークン(`)`、または`(`がない場合はメンバー名)を指す
    /// (`struct_init_node`が最後の`}`を指して終わるのと同じ)
    pub(in crate::compiler::parse)
    fn variant_init_node<const T: bool>(
        &mut self,
        name: &str,
    ) -> Result<node::Expr, err::ErrKind> {
        let member = match self.current_tkn() {
            lex::Tkn::Name(member) => member.to_string(),
            t => {
                return crate::err_at!(self.variant_init_member_not_found((*t).clone()));
            }
        };

        // `(`が続かなければ、データなしのメンバー(`Variant::Name`)
        if self.peek_tkn()? != lex::Tkn::LParen {
            return Ok(node::Expr::EnumVariant {
                name: name.to_string(),
                variant: member,
            });
        }

        // `(`へ進み、さらに値の先頭のトークンへ進む
        let _ = self.next_tkn(&["("])?;
        let _ = self.next_tkn(&[])?;
        let value = Box::new(self.expr_cmp(true)?);

        // 値の後ろは`)`でなければならない
        if self.current_tkn() != &lex::Tkn::RParen {
            crate::err_at!(self.variant_in_unexpect_tkn(lex::Tkn::RParen))?;
        }

        let mut fields = HashMap::<String, Box<node::Expr>>::new();
        fields.insert(member, value);
        Ok(node::Expr::InitVariant {
            is_self: T,
            name: name.to_string(),
            fields,
        })
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
            return crate::err_at!(self.ty_colon_not_found());
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
                        // ジェネリクスな構造体/バリアント: `Name<int>`
                        lex::Tkn::LAngleBracket => {
                            if !self.generic_types.contains_key(&name) {
                                return crate::err_at!(self.generics_not_supported_yet());
                            }
                            // その型引数の構造体/バリアントを(まだ無ければ)作り、
                            // 作った型の名前の型にする。終了時は`>`を指す
                            let generic_name = self.generic_type_ref(&name)?;
                            let generic_ty = self.is_constract_ty(node::TyNode::Ty(generic_name))?;
                            match self.next_tkn(&["*", "["])? {
                                lex::Tkn::LBracket => {
                                    self.next_tkn(&["["])?;
                                    self.make_range_ptr_node(generic_ty)?
                                }
                                lex::Tkn::Mul => self.ptr_ty_node(generic_ty)?,
                                _ => generic_ty,
                            }
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
    use crate::compiler::node::Field;
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
    fn test_variant() {
        assert_eq!(
            &build("variant Name { A B(ty) }"),
            &vec![node::VariantDefine::new(
                "Name".to_string(),
                vec![
                    node::VariantField::typeless_new("A".to_string()),
                    node::VariantField::make_field("B", "ty"),
                ],
                Vec::new()
            )]
        );
    }

    #[test]
    fn test_variant_last_typeless() {
        // 最後のメンバーが型なしでも、`}`で終われる
        assert_eq!(
            &build("variant Name { A }"),
            &vec![node::VariantDefine::new(
                "Name".to_string(),
                vec![node::VariantField::typeless_new("A".to_string())],
                Vec::new()
            )]
        );
    }

    #[test]
    fn test_variant_unsafe() {
        assert_eq!(
            &build("variant \"unsafe\" Name { a: int b: i64 }"),
            &vec![node::VariantDefine::new(
                "Name".to_string(),
                vec![
                    node::VariantField::unsafe_new("a".to_string(), node::TyNode::Ty("int".to_string())),
                    node::VariantField::unsafe_new("b".to_string(), node::TyNode::Ty("i64".to_string())),
                ],
                Vec::new()
            )]
        );
    }

    #[test]
    fn variant_method() {
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
            &build("variant Name { A(int) new(): ty {}}"),
            &vec![node::VariantDefine::new(
                "Name".to_string(),
                vec![node::VariantField::make_field("A", "int")],
                f
            )]
        );
    }

    #[test]
    fn variant_method_self_ty() {
        // 共用体でも`Self`は、定義されている共用体自身の名前へ解決される
        let nodes = build(
            "variant Name { \
                A(int) \
                new(): Self {} \
                func(self: Self, param: int): int {ret 0} \
            }",
        );
        let node::Group1Node::VariantDefine(variant_def) = &nodes[0] else {
            panic!("共用体が定義されていません: {:?}", nodes);
        };

        let node::Group1Node::FuncDefine(new_func) = &variant_def.methods[0] else {
            panic!();
        };
        assert_eq!(new_func.name, "new");
        assert_eq!(new_func.ret_ty, node::TyNode::SelfTy("Name".to_string()));

        let node::Group1Node::FuncDefine(func) = &variant_def.methods[1] else {
            panic!();
        };
        assert_eq!(func.name, "func");
        assert_eq!(func.params[0].ty, node::TyNode::SelfTy("Name".to_string()));
        assert_eq!(func.params[0].name, "self");
        assert_eq!(func.params[1].name, "param");
        assert_eq!(func.params[1].ty, node::TyNode::Ty("int".to_string()));
    }

    #[test]
    fn variant_name_is_not_found() {
        assert!(build_is_err("variant { A }"));
    }

    #[test]
    fn variant_lbrace_not_found() {
        assert!(build_is_err("variant Name A }"));
    }

    #[test]
    fn enum_like_variant() {
        // 旧`enum Name {A B}`は、型なしメンバーだけの`variant`になる
        assert_eq!(
            &build("variant Name {A B}"),
            &vec![node::VariantDefine::new(
                "Name".to_string(),
                vec![
                    node::VariantField::typeless_new("A".to_string()),
                    node::VariantField::typeless_new("B".to_string()),
                ],
                Vec::new()
            )]
        );
    }

    #[test]
    fn enum_like_variant_with_comma() {
        // 旧`enum`の`,`区切りも使える
        assert_eq!(
            &build("variant Name {A, B,}"),
            &build("variant Name {A B}")
        );
    }

    #[test]
    fn variant_enum_like_with_method() {
        // 旧`enum`ではメゾットが捨てられていたが、統合後は保持される
        let nodes = build("variant Name { A B func(self: Self): int {ret 0} }");
        let node::Group1Node::VariantDefine(def) = &nodes[0] else {
            panic!("variant が定義されていません: {:?}", nodes);
        };
        assert_eq!(def.fields.len(), 2);
        assert_eq!(def.methods.len(), 1);
        assert!(def.is_plain_enum());
    }

    #[test]
    fn variant_with_data_is_not_plain_enum() {
        let nodes = build("variant Name { A B(int) }");
        let node::Group1Node::VariantDefine(def) = &nodes[0] else {
            panic!();
        };
        assert!(!def.is_plain_enum());
        assert!(def.is_tagged());
    }

    #[test]
    fn variant_unsafe_not_plain_enum() {
        let nodes = build("variant \"unsafe\" Name { a: int b: i64 }");
        let node::Group1Node::VariantDefine(def) = &nodes[0] else {
            panic!();
        };
        assert!(!def.is_tagged());
        assert!(!def.is_plain_enum());
    }

    #[test]
    fn variant_unregistered_option() {
        assert!(build_is_err("variant \"safe\" Name { A }"));
    }
}