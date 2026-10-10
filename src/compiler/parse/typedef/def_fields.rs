//! 構造体や共用体などのフィールドを生成するAPIを提供

use crate::compiler::node::{Field, Group1Node, StructField, VariantField, VariantFieldKind, VariantMode};

use super::*;


const LOOP_BRAEK: bool = true;


impl Parser {
    /// 構造体のメンバ定義を解析する関数
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace`
    pub(in crate::compiler::parse::typedef)
    fn define_struct_fields(
        &mut self,
    ) -> Result<(Vec<node::StructField>, Vec<node::Group1Node>), err::ErrKind> {
        self.define_colon_fields::<StructField>()
    }

    /// `name: ty`形式のメンバーとメゾットを解析する共通処理
    /// (構造体と、`unsafe`の共用体で使う)
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace`
    fn define_colon_fields<T: Field>(
        &mut self,
    ) -> Result<(Vec<T>, Vec<node::Group1Node>), err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return crate::err_at!(self.fields_lbrace_not_found());
        }

        let mut fields = Vec::<T>::new();
        let mut pub_flag = false;
        let mut methods = Vec::new();

        loop {
            match self.next_tkn(&["name", "pub", "}"])? {
                lex::Tkn::Name(name) => {
                    match self.next_tkn_ref(&[":", "("])? {
                        lex::Tkn::Colon => {
                            if self.make_field_member_ty::<T>(&mut fields, name)? {
                                break;
                            }
                            continue;
                        }

                        lex::Tkn::LParen => {
                            let mut method = self.func_node(name, pub_flag)?;
                            self.next_tkn(&[])?;
                            self.scope_counter += 1;
                            // メゾットの本体が空(`{}`)の場合、`one_line_node`を
                            // 呼ばずにそのまま本体無しとして扱う
                            self.make_field_method_node(&mut method)?;

                            self.scope_counter -= 1;
                            // モジュールの名前を登録
                            self.registration_mod_name(&mut method);
                            methods.push(method);

                            // メゾットの本体を閉じる`}`の次のトークンは、
                            // 次のメンバーの`name`/`pub`、または構造体を
                            // 閉じる`}`のいずれか。すでにそのトークンを
                            // 指しているので、`Colon`アームと違い
                            // 下の共通処理(`next_tkn`での読み進め)は
                            // 行わずここで直接分岐する
                            match self.current_tkn() {
                                lex::Tkn::RBrace => {
                                    //self.advance_tkn().unwrap();
                                    break;
                                }
                                lex::Tkn::Name(_) | lex::Tkn::KeyWordPub => {
                                    self.back_tkn();
                                }
                                t => panic!("{:?}", t),
                            }
                            pub_flag = false;
                            continue;
                        }

                        _ => return crate::err_at!(self.field_colon_or_lparen_not_found()),
                    }
                    pub_flag = false;
                    self.next_tkn(&[])?;
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


    /// 共用体のメンバーのノードを作成
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace`
    /// - `Normal`: `Mem` / `Mem(ty)` / メゾット(タグ付き共用体)
    /// - `Unsafe`: `mem: ty`(構造体と同じ書き方。タグを持たない)
    pub(in crate::compiler::parse::typedef)
    fn define_variant_fields(
        &mut self,
        variant_flag: VariantMode,
    ) -> Result<(Vec<node::VariantField>, Vec<node::Group1Node>), err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return crate::err_at!(self.fields_lbrace_not_found());
        }

        match variant_flag {
            VariantMode::Normal => self.safe_variant_field(),
            VariantMode::Unsafe => {
                let (mut fields, methods) = self.define_colon_fields::<VariantField>()?;
                for field in fields.iter_mut() {
                    field.mode = VariantMode::Unsafe;
                }
                Ok((fields, methods))
            }
        }
    }

    /// ラベル付きの共用体を作成
    /// ```text
    /// variant A { Mem  Mem1(int)  func(self: Self) {} }
    /// ```
    fn safe_variant_field(
        &mut self,
    ) -> Result<(Vec<node::VariantField>, Vec<node::Group1Node>), err::ErrKind> {
        let mut fields = Vec::<node::VariantField>::new();
        let mut methods = Vec::new();
        let mut pub_flag = false;

        loop {
            match self.next_tkn(&["name", "pub", "}"])? {
                lex::Tkn::RBrace => {
                    // スコープが終了
                    break;
                }
                lex::Tkn::KeyWordPub => {
                    pub_flag = true;
                }
                // メンバーの区切りの`,`は省略できる
                lex::Tkn::Comma => {}
                // フィールドの名前を取得
                lex::Tkn::Name(field_name) => {
                    match self.make_variant_field(field_name, pub_flag)? {
                        VariantFieldKind::Field(field) => fields.push(field),
                        VariantFieldKind::Method(method) => methods.push(method),
                    }
                    pub_flag = false;
                }
                t => {
                    return crate::err_at!(self.variant_member_not_found(t));
                }
            }
        }

        Ok((fields, methods))
    }

    /// 呼び出しもとで`:`が来たらそれは構造体などのメンバー
    /// メンバーのノードを作成する。
    /// ## Args
    /// - fields
    /// - name // 所有権で渡す
    fn make_field_member_ty<T: Field>(&mut self, fields: &mut Vec<T>, name: String) -> Result<bool, err::ErrKind> {
        self.next_tkn(&[])?;
        let ty = self.define_ty_node()?;

        fields.push(T::new(
            name,
            ty,
        ));

        match self.current_tkn() {
            lex::Tkn::Comma => {}
            lex::Tkn::RBrace => return Ok(LOOP_BRAEK),
            // `,`を省略して改行だけで次のメンバーへ続く場合
            // (現在のトークンが次のメンバーの`name`/`pub`を
            // 指しているので、位置を1つ戻して、ループの先頭の
            // `next_tkn`でもう一度読めるようにする)
            lex::Tkn::Name(_) | lex::Tkn::KeyWordPub => {
                self.back_tkn();
            }
            t => return crate::err_at!(self.field_end_unexpected_tkn(t.clone())),
        }
        Ok(false)
    }

    /// 構造体などの中にメゾットを登録
    pub(in crate::compiler::parse::typedef)
    fn registration_mod_name(&self, method: &mut Group1Node) {
        // モジュールの名前を登録
        if let node::Group1Node::FuncDefine(func) = method {
            if let Some(mod_name) = &self.struct_self_name {
                func.self_module_name(mod_name.to_owned());
            } else {
                panic!();
            }
        } else {
            panic!();
        }
    }

    /// メゾットの中身を作成
    pub(in crate::compiler::parse::typedef) 
    fn make_field_method_node(&mut self, method: &mut Group1Node) -> Result<(), err::ErrKind> {
        // メゾットの本体が空(`{}`)の場合、`one_line_node`を
        // 呼ばずにそのまま本体無しとして扱う
        if self.current_tkn() != &lex::Tkn::RBrace {
            loop {
                if let node::Group1Node::FuncDefine(func) = method {
                    func.body
                        .push(self.one_line_node()?.gen_group_info(self.build_err_span().line));
                }

                if self.current_tkn() == &lex::Tkn::RBrace {
                    self.advance_tkn().unwrap();
                    break;
                }
            }
        } else {
            self.advance_tkn().unwrap();
        }
        Ok(())
    }
}