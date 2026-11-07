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
        let range = match self.peek_tkn()? {
            lex::Tkn::LParen => Some((0usize, 0usize)),
            lex::Tkn::Str(s) if s.as_str() == "unlimit" => {
                self.advance_tkn().unwrap();
                Some((0usize, 0xFFFF_FFFF_FFFF_FFFFusize))
            }
            _ => None,
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
        match self
            .next_tkn(&["name", "[", "string", "Self", "="])?
            .clone()
        {
            lex::Tkn::Equal => Ok(TyNode::Auto),
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
            lex::Tkn::Name(mut name) => {
                // `mod::Type` / `mod::Type<int>`: モジュール名は型の名前に含めない
                // (`#include`先のジェネリクスな型は、ここで取り込む)
                while matches!(self.peek_tkn(), Ok(lex::Tkn::ModPathTkn)) {
                    // `::`、型の名前の順に進める
                    self.next_tkn(&["::"])?;
                    let found = self.next_tkn(&["name"])?;
                    let lex::Tkn::Name(next_name) = found.clone() else {
                        return crate::err_at!(self.expected_tkn("name", found));
                    };
                    let module = std::mem::replace(&mut name, next_name);
                    self.import_included_generic_type(&module, &name);
                }
                // 契約型かを調べる
                let base_ty = self.is_constract_ty(TyNode::Ty(name.clone()))?;
                let ty =
                    match self.next_tkn(&["<", "*", "["])? {
                        // ジェネリクスな構造体/バリアント: `Name<int>`
                        lex::Tkn::LAngleBracket => {
                            let generic_ty = if self.generic_types.contains_key(&name) {
                                // その型引数の構造体/バリアントを(まだ無ければ)作り、
                                // 作った型の名前の型にする。終了時は`>`を指す
                                let generic_name = self.generic_type_ref(&name)?;
                                self.is_constract_ty(TyNode::Ty(generic_name))?
                            } else if self.generic_aliases.contains_key(&name) {
                                // ジェネリクスな型エイリアス: `Alias<int>`を、
                                // 型パラメータを置き換えた型にする。終了時は`>`を指す
                                self.generic_alias_ref(&name)?
                            } else {
                                // ジェネリクスな型でも型エイリアスでもない型に、型引数が付いている
                                return crate::err_at!(self.generics_not_supported_yet());
                            };
                            match self.next_tkn(&["*", "["])? {
                                lex::Tkn::LBracket => {
                                    self.next_tkn(&["["])?;
                                    self.make_range_ptr_node(generic_ty)?
                                }
                                lex::Tkn::Mul => {
                                    let ptr_ty = self.ptr_ty_node(generic_ty)?;
                                    // `Pair<int>* must=free`
                                    self.contract_after_ty(ptr_ty)?
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
                            self.contract_after_ty(ptr_ty)?
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
mod ty_tests;
