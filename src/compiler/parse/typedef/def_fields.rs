//! 構造体や共用体などのフィールドを生成するAPIを提供

use crate::compiler::node::{Field, Group1Node, StructField, UnionField};

use super::*;


const LOOP_BRAEK: bool = true;

impl Parser {
    /// 構造体のメンバ定義を解析する関数
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace`
    pub(in crate::compiler::parse::typedef)
    fn define_struct_fields(
        &mut self,
    ) -> Result<(Vec<node::StructField>, Vec<node::Group1Node>), err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return Err(err::ErrKind::NotFoundTkn(Box::new(lex::Tkn::LBrace)));
        }

        let mut fields = Vec::<node::StructField>::new();
        let mut pub_flag = false;
        let mut methods = Vec::new();

        loop {
            match self.next_tkn(&["name", "pub", "}"])? {
                lex::Tkn::Name(name) => {
                    match self.next_tkn_ref(&[":", "("])? {
                        lex::Tkn::Colon => {
                            if self.make_field_member_ty::<StructField>(&mut fields, name)? {
                                break;
                            }
                            continue;
                        }

                        lex::Tkn::LParen => {
                            let mut method = self.func_node(&name, pub_flag)?;
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

                        _ => return Err(err::ErrKind::UnexpectedToken),
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
    /// 構造体のメンバ定義を解析する関数
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace`
    pub(in crate::compiler::parse::typedef)
    fn define_union_fields(
        &mut self,
    ) -> Result<(Vec<node::UnionField>, Vec<node::Group1Node>), err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return Err(err::ErrKind::NotFoundTkn(Box::new(lex::Tkn::LBrace)));
        }

        let mut fields = Vec::<node::UnionField>::new();
        let mut pub_flag = false;
        let mut methods = Vec::new();

        loop {
            match self.next_tkn(&["name", "pub", "}"])? {
                lex::Tkn::Name(name) => {
                    match self.next_tkn_ref(&[":", "("])? {
                        lex::Tkn::Colon => {
                            if self.make_field_member_ty::<UnionField>(&mut fields, name)? {
                                break;
                            }
                            continue;
                        }
                        lex::Tkn::LParen => {
                            let mut method = self.func_node(&name, pub_flag)?;
                            self.next_tkn(&[])?;
                            self.scope_counter += 1;

                            self.make_field_method_node(&mut method)?;
                            
                            self.scope_counter -= 1;
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

                        _ => return Err(err::ErrKind::UnexpectedToken),
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
            t => crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::UnexpectTknAfterKeyword {
                    keyword: lex::Tkn::Colon,
                    expected: vec![",", "}"],
                    found: t.clone(),
                }
            )?,
        }
        Ok(false)
    }

    /// 構造体などをモジュールとして登録
    fn registration_mod_name(&self, method: &mut Group1Node) {
        // モジュールの名前を登録
        if let node::Group1Node::FuncDefine(func) = method {
            func.self_module_name(self.struct_self_name.as_ref().unwrap());
        } else {
            panic!();
        }
    }

    /// メゾットの中身を作成
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