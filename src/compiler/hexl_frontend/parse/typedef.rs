mod def_fields;
mod generic;
mod impl_block;
pub(in crate::compiler::hexl_frontend::parse) mod ty_alias;
mod variant_field;

use super::{Parser, *};
use std::collections::HashMap;
use std::rc::Rc;

/// 構造体、バリアント型(共用体と列挙型を統合したもの)を定義するノードの生成
impl Parser {
    /// `struct name { mem: ty, mem2: ty2 }` を解析する
    /// 呼び出し時は current_tkn() が KeyWordStruct
    pub(in crate::compiler::hexl_frontend::parse) fn struct_node(
        &mut self,
    ) -> Result<Group1Node, err::ErrKind> {
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

        let (fields, mut methods) = self.define_struct_fields()?;
        // 構造体の中身をすべて処理し終わったので、`None`にする
        self.struct_self_name = None;
        self.gen_flag = GenFlag::Group1;
        // この構造体より先に`impl 名前 { .. }`が書かれていたら、そのメゾットを追加する
        if let Some(reserved) = self.reserved_impls.remove(&name) {
            methods.extend(reserved);
        }
        let r = StructDefine::new(name, fields, methods, self.build_err_span());
        Ok(r)
    }

    /// バリアント型(旧`union`と旧`enum`を統合したもの)を解析する
    /// ```text
    /// variant Name { A B }                                  // 列挙型相当
    /// variant Name { Mem  Mem1(int)  func(self: Self): int {..} }
    /// variant "unsafe" Name { mem: int  mem2: i64 }        // タグなし(旧unsafe union)
    /// ```
    /// 呼び出し時は current_tkn() が KeyWordVariant
    pub(in crate::compiler::hexl_frontend::parse) fn variant_node(
        &mut self,
    ) -> Result<Group1Node, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::KeyWordVariant {
            return crate::err_at!(self.variant_keyword_not_found());
        }

        let (name, variant_mode) = match self.next_tkn(&["name", "str litral"])? {
            lex::Tkn::Name(variant_name) => (variant_name, VariantMode::Normal),
            lex::Tkn::Str(variant_option) => match variant_option.as_str() {
                "unsafe" => match self.next_tkn(&["name"])? {
                    lex::Tkn::Name(variant_name) => (variant_name, VariantMode::Unsafe),
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

        let (fields, mut methods) = self.define_variant_fields(variant_mode)?;
        // 共用体の中身をすべて処理し終わったので、`None`にする
        self.struct_self_name = None;
        self.gen_flag = GenFlag::Group1;
        // このバリアントより先に`impl 名前 { .. }`が書かれていたら、そのメゾットを追加する
        if let Some(reserved) = self.reserved_impls.remove(&name) {
            methods.extend(reserved);
        }
        Ok(VariantDefine::new(
            name,
            fields,
            methods,
            self.build_err_span(),
        ))
    }

    /// 構造体を初期化する式を生成
    pub(in crate::compiler::hexl_frontend::parse) fn struct_init_node<const T: bool>(
        &mut self,
        name: &str,
    ) -> Result<Expr, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            crate::err_at!(self.struct_lbrace_not_found(self.current_tkn().clone()))?;
        }
        // {を飛ばす
        let _ = self.next_tkn(&[])?;
        let mut fields = HashMap::<String, Box<Expr>>::new();

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
                Box::new(self.expr_or(true)?),
            );

            match self.current_tkn() {
                lex::Tkn::Name(..) => {
                    continue;
                }
                lex::Tkn::RBrace => break,
                t => {
                    return crate::err_at!(self.expected_tkn("name or `}`", t.clone()));
                }
            }
        }
        Ok(Expr::new(
            ExprNode::InitStruct {
                is_self: T,
                name: name.to_string(),
                fields: fields.clone(),
            },
            self.build_err_span(),
        ))
    }

    /// バリアント型を初期化する式を生成
    /// ```text
    /// Variant::Name(expr)   // データ付きメンバー -> `Expr::InitVariant`
    /// Variant::Name         // データなしメンバー -> `Expr::EnumVariant`
    /// ```
    /// 呼び出し時は current_tkn() が `Variant::`の次の`Name`(メンバー名)
    /// 終了時は、式の最後のトークン(`)`、または`(`がない場合はメンバー名)を指す
    /// (`struct_init_node`が最後の`}`を指して終わるのと同じ)
    pub(in crate::compiler::hexl_frontend::parse) fn variant_init_node<const T: bool>(
        &mut self,
        name: &str,
    ) -> Result<Expr, err::ErrKind> {
        let member = match self.current_tkn() {
            lex::Tkn::Name(member) => member.to_string(),
            t => {
                return crate::err_at!(self.variant_init_member_not_found((*t).clone()));
            }
        };

        // `(`が続かなければ、データなしのメンバー(`Variant::Name`)
        if self.peek_tkn()? != lex::Tkn::LParen {
            return Ok(Expr::new(
                ExprNode::EnumVariant {
                    name: name.to_string(),
                    variant: member,
                },
                self.build_err_span(),
            ));
        }

        // `(`へ進み、さらに値の先頭のトークンへ進む
        let _ = self.next_tkn(&["("])?;
        let value = Box::new(self.expr_or(true)?);

        // 値の後ろは`)`でなければならない
        if self.current_tkn() != &lex::Tkn::RParen {
            crate::err_at!(self.variant_in_unexpect_tkn(lex::Tkn::RParen))?;
        }

        let mut fields = HashMap::<String, Box<Expr>>::new();
        fields.insert(member, value);
        Ok(Expr::new(
            ExprNode::InitVariant {
                is_self: T,
                name: name.to_string(),
                fields,
            },
            self.build_err_span(),
        ))
    }

    fn ptr_ty_node(&mut self, ty: TyNode) -> Result<TyNode, err::ErrKind> {
        let range = if self.peek_tkn()? == lex::Tkn::LParen {
            // ポインタの範囲指定がある場合、`(start, end)`を読み込む
            Some((0usize, 0usize))
        } else {
            None
        };
        let is_const = self.is_const_ptr()?;
        self.next_tkn(&[])?;

        Ok(TyNode::Pointer {
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
    /// `TyNode::SelfTy(self_name)`へ解決できる。
    /// メゾットの外(トップレベルの関数など)では`None`を渡す。
    pub(in crate::compiler::hexl_frontend::parse) fn define_ty_node(
        &mut self,
    ) -> Result<TyNode, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::Colon {
            return crate::err_at!(self.ty_colon_not_found());
        }
        self.ty_node_after_delim()
    }

    /// 型のノードを作成する。`define_ty_node`と違い、型の1つ手前の
    /// トークンが`:`である必要はなく、`current_tkn()`は型の直前の
    /// 区切り(`:`、ジェネリクスの型引数の`<`や`,`)を指していればよい。
    /// 終了時は、型の次のトークンを指す
    pub(in crate::compiler::hexl_frontend::parse) fn ty_node_after_delim(
        &mut self,
    ) -> Result<TyNode, err::ErrKind> {
        match self.next_tkn(&["name", "[", "string", "Self", "="])?.clone() {
            lex::Tkn::Equal => {
                Ok(TyNode::Auto)
            }
            // 予約語`Self`: 自身の構造体を指す型
            lex::Tkn::KeyWordSelf => {
                // `Self`の次のトークンへ進める
                self.next_tkn(&[])?;

                let Some(self_name) = self.struct_self_name.clone() else {
                    return crate::err_at!(self.self_outside_of_type());
                };

                let ty = match self.current_tkn() {
                    lex::Tkn::Mul => {
                        // ポインタ化する場合はここで ty を組み立てる
                        let ty = TyNode::SelfTy(self_name.clone());
                        self.ptr_ty_node(ty)?
                    }
                    _ => TyNode::SelfTy(self_name.clone()),
                };

                Ok(ty)
            }
            lex::Tkn::Name(name) => {
                // 契約型かを調べる
                let base_ty = self.is_constract_ty(TyNode::Ty(name.clone()))?;
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
                            let generic_ty = self.is_constract_ty(TyNode::Ty(generic_name))?;
                            match self.next_tkn(&["*", "["])? {
                                lex::Tkn::LBracket => {
                                    self.next_tkn(&["["])?;
                                    self.make_range_ptr_node(generic_ty)?
                                }
                                lex::Tkn::Mul => {
                                    let ptr_ty = self.ptr_ty_node(generic_ty)?;
                                    // `Pair<int>* must=free`
                                    self.constract_after_ty(ptr_ty)?
                                }
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
                        lex::Tkn::Mul => {
                            let ptr_ty = self.ptr_ty_node(base_ty)?;
                            // `int* must=free` / `int* of free`
                            self.constract_after_ty(ptr_ty)?
                        }
                        _ => {
                            // ジェネリクスに定義ずみの型がある
                            if self.other_stk.iter().any(|(stk_name, info)| {
                                stk_name == &name && info == &StkInfo::Generics
                            }) {
                                TyNode::make_ref_ty(&name)
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
                let t = self.next_tkn(&["["])?;
                if t != lex::Tkn::LBracket {
                    return crate::err_at!(self.expected_tkn("[", t));
                }
                self.define_mem_ty_node(true)
            }
            t => crate::err_at!(self.expected_tkn("name, `[` or `static`", t)),
        }
    }

    /// スタック/静的領域に確保する変数の型を解析する
    /// 呼び出し時、`current_tkn()`は`LBracket`
    /// - `[ty]`   -> len == 1
    /// - `[ty N]` -> len == N
    ///
    /// 呼び出し終了時は、`]`の次のトークンを指す
    fn define_mem_ty_node(&mut self, is_static: bool) -> Result<TyNode, err::ErrKind> {
        let ty = self.ty_node_after_delim()?;
        self.back_tkn();

        let len = match self.next_tkn(&["number", "]"])? {
            lex::Tkn::Number(num) => {
                let Ok(len) = num.parse::<usize>() else {
                    return crate::err_at!(self.invalid_number(num));
                };

                let t = self.next_tkn(&["not `]`"])?;
                if t != lex::Tkn::RBracket {
                    return crate::err_at!(self.expected_tkn("]", t));
                }
                len
            }
            lex::Tkn::RBracket => 1,
            t => return crate::err_at!(self.expected_tkn("number or `]`", t)),
        };
        // `]`の次のトークンに進める
        // (呼び出し元が`=`などを current_tkn() で判定できるように)
        self.next_tkn(&[])?;

        if is_static {
            Ok(TyNode::Static {
                name: Box::new(ty),
                len,
            })
        } else {
            Ok(TyNode::Stack {
                name: Box::new(ty),
                len,
            })
        }
    }
}

#[cfg(test)]
mod ty_tests {
    use super::*;
    use crate::compiler::Field;
    use crate::compiler::ast::zero_span::ZeroSpan;
    use std::collections::HashMap;
    use std::rc::Rc;

    /// テストで組み立てるノードに付ける位置(`line`も`pos`も0)
    fn sp() -> Rc<err::Span> {
        Rc::new(err::Span::new(0, 0, String::new()))
    }

    /// パース結果。すべての`span`と`Group2Info.line`を0にして返す
    fn build(value: &str) -> Vec<Group1Node> {
        let mut lex = lex::Lexer::new();
        let _ = lex.analy(&value.to_string()).unwrap();
        let mut parse = Parser::new_for_test();
        parse
            .parser_for_test(&lex.gen_tkns.clone())
            .unwrap()
            .to_vec()
            .zeroed_spans()
    }

    /// `unsafe`共用体のメンバー(`mode`が`Unsafe`)を作る
    fn unsafe_field(name: &str, ty: &str) -> VariantField {
        let mut field = VariantField::new(name.to_string(), TyNode::Ty(ty.to_string()), sp());
        field.mode = VariantMode::Unsafe;
        field
    }

    /// 構文解析がエラーになるかどうか
    fn build_is_err(value: &str) -> bool {
        let mut lex = lex::Lexer::new();
        let _ = lex.analy(&value.to_string()).unwrap();
        let mut parse = Parser::new_for_test();
        parse.parser_for_test(&lex.gen_tkns.clone()).is_err()
    }

    #[test]
    fn test_struct() {
        assert_eq!(
            &build("struct Name { name: ty name2: ty }"),
            &vec![StructDefine::new(
                "Name".to_string(),
                vec![
                    StructField::make_field("name", "ty", sp()),
                    StructField::make_field("name2", "ty", sp())
                ],
                Vec::new(),
                sp()
            )]
        );
    }

    #[test]
    fn struct_init() {
        let mut func = FuncDefine::new(
            "main".to_string(),
            Vec::new(),
            TyNode::Ty("int".to_string()),
            false,
            sp(),
        );
        let map: HashMap<String, Box<Expr>> = [
            (
                "name".to_string(),
                Box::new(ExprNode::Number("1".to_string()).with_span(sp())),
            ),
            (
                "name2".to_string(),
                Box::new(ExprNode::Number("1".to_string()).with_span(sp())),
            ),
        ]
        .into_iter()
        .collect();
        let Group1Node::FuncDefine(ref mut f) = func else {
            panic!();
        };
        f.add(
            Group2Node::Expr(
                ExprNode::InitStruct {
                    is_self: false,
                    name: "Name".to_string(),
                    fields: map,
                }
                .with_span(sp()),
            )
            .gen_group_info(0, sp()),
        );
        assert_eq!(
            &build("main(): int { Name { name: 1 name2: 1 } }"),
            &vec![func]
        );
    }

    #[test]
    fn struct_method() {
        let mut f = vec![FuncDefine::new(
            "new".to_string(),
            Vec::new(),
            TyNode::Ty("ty".to_string()),
            false,
            sp(),
        )];
        let Group1Node::FuncDefine(func) = f.last_mut().unwrap() else {
            panic!();
        };
        func.self_module_name("Name".to_string());
        assert_eq!(
            &build("struct Name { a: int new(): ty {}}"),
            &vec![StructDefine::new(
                "Name".to_string(),
                vec![StructField::make_field("a", "int", sp()),],
                f,
                sp()
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
        let Group1Node::StructDefine(struct_def) = &nodes[0] else {
            panic!("構造体が定義されていません: {:?}", nodes);
        };

        let Group1Node::FuncDefine(new_func) = &struct_def.methods[0] else {
            panic!();
        };
        assert_eq!(new_func.name, "new");
        assert_eq!(new_func.ret_ty, TyNode::SelfTy("Name".to_string()));

        let Group1Node::FuncDefine(func) = &struct_def.methods[1] else {
            panic!();
        };
        assert_eq!(func.name, "func");
        assert_eq!(func.params[0].ty, TyNode::SelfTy("Name".to_string()));
        assert_eq!(func.params[0].name, "self");
        assert_eq!(func.params[1].name, "param");
        assert_eq!(func.params[1].ty, TyNode::Ty("int".to_string()));
    }

    #[test]
    fn test_variant() {
        assert_eq!(
            &build("variant Name { A B(ty) }"),
            &vec![VariantDefine::new(
                "Name".to_string(),
                vec![
                    VariantField::typeless_new("A".to_string(), sp()),
                    VariantField::make_field("B", "ty", sp()),
                ],
                Vec::new(),
                sp()
            )]
        );
    }

    #[test]
    fn test_variant_last_typeless() {
        // 最後のメンバーが型なしでも、`}`で終われる
        assert_eq!(
            &build("variant Name { A }"),
            &vec![VariantDefine::new(
                "Name".to_string(),
                vec![VariantField::typeless_new("A".to_string(), sp())],
                Vec::new(),
                sp()
            )]
        );
    }

    #[test]
    fn test_variant_unsafe() {
        assert_eq!(
            &build("variant \"unsafe\" Name { a: int b: i64 }"),
            &vec![VariantDefine::new(
                "Name".to_string(),
                vec![unsafe_field("a", "int"), unsafe_field("b", "i64")],
                Vec::new(),
                sp()
            )]
        );
    }

    #[test]
    fn variant_method() {
        let mut f = vec![FuncDefine::new(
            "new".to_string(),
            Vec::new(),
            TyNode::Ty("ty".to_string()),
            false,
            sp(),
        )];
        let Group1Node::FuncDefine(func) = f.last_mut().unwrap() else {
            panic!();
        };
        func.self_module_name("Name".to_string());
        assert_eq!(
            &build("variant Name { A(int) new(): ty {}}"),
            &vec![VariantDefine::new(
                "Name".to_string(),
                vec![VariantField::make_field("A", "int", sp())],
                f,
                sp()
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
        let Group1Node::VariantDefine(variant_def) = &nodes[0] else {
            panic!("共用体が定義されていません: {:?}", nodes);
        };

        let Group1Node::FuncDefine(new_func) = &variant_def.methods[0] else {
            panic!();
        };
        assert_eq!(new_func.name, "new");
        assert_eq!(new_func.ret_ty, TyNode::SelfTy("Name".to_string()));

        let Group1Node::FuncDefine(func) = &variant_def.methods[1] else {
            panic!();
        };
        assert_eq!(func.name, "func");
        assert_eq!(func.params[0].ty, TyNode::SelfTy("Name".to_string()));
        assert_eq!(func.params[0].name, "self");
        assert_eq!(func.params[1].name, "param");
        assert_eq!(func.params[1].ty, TyNode::Ty("int".to_string()));
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
            &vec![VariantDefine::new(
                "Name".to_string(),
                vec![
                    VariantField::typeless_new("A".to_string(), sp()),
                    VariantField::typeless_new("B".to_string(), sp()),
                ],
                Vec::new(),
                sp()
            )]
        );
    }

    #[test]
    fn enum_like_variant_with_comma() {
        // 旧`enum`の`,`区切りも使える
        assert_eq!(&build("variant Name {A, B,}"), &build("variant Name {A B}"));
    }

    #[test]
    fn variant_enum_like_with_method() {
        // 旧`enum`ではメゾットが捨てられていたが、統合後は保持される
        let nodes = build("variant Name { A B func(self: Self): int {ret 0} }");
        let Group1Node::VariantDefine(def) = &nodes[0] else {
            panic!("variant が定義されていません: {:?}", nodes);
        };
        assert_eq!(def.fields.len(), 2);
        assert_eq!(def.methods.len(), 1);
        assert!(def.is_plain_enum());
    }

    #[test]
    fn variant_with_data_is_not_plain_enum() {
        let nodes = build("variant Name { A B(int) }");
        let Group1Node::VariantDefine(def) = &nodes[0] else {
            panic!();
        };
        assert!(!def.is_plain_enum());
        assert!(def.is_tagged());
    }

    #[test]
    fn variant_unsafe_not_plain_enum() {
        let nodes = build("variant \"unsafe\" Name { a: int b: i64 }");
        let Group1Node::VariantDefine(def) = &nodes[0] else {
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