//! `impl Name { func() {..} }`を解析するAPIを提供
//!
//! - `Name`の`struct`/`variant`が先に定義されている: その型のメゾットに追加する
//! - まだ定義されていない: メゾットを`reserved_impls`に予約しておき、
//!   `Name`が定義されたとき(`struct_node`/`variant_node`)に追加する。
//!   最後まで定義されなければ、`Parser::parser`がエラーにする

use super::*;

impl Parser {
    /// `impl Name { メゾット.. }`を解析する
    /// 呼び出し時は current_tkn() が `KeyWordImpl`、
    /// 終了時は`impl`を閉じる`}`を指す(`struct_node`と同じ)
    pub(in crate::compiler::hexl_frontend::parse) fn impl_node(
        &mut self,
    ) -> Result<(), err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::KeyWordImpl {
            return crate::err_at!(self.expected_at_current("impl"));
        }
        let lex::Tkn::Name(name) = self.next_tkn(&["name"])? else {
            return crate::err_at!(self.expected_at_current("name"));
        };
        if self.next_tkn(&["{"])? != lex::Tkn::LBrace {
            return crate::err_at!(self.expected_at_current("{"));
        }

        // メゾットの中の`Self`を`name`に解決するため、自身の名前を登録する
        self.struct_self_name = Some(name.clone());
        let methods = self.define_impl_methods()?;
        self.struct_self_name = None;
        self.gen_flag = GenFlag::Group1;

        // すでに定義されている構造体/バリアントを探す
        let target = self
            .gen_nodes
            .iter_mut()
            .chain(self.pending_types.iter_mut())
            .find_map(|node| match node {
                Group1Node::StructDefine(def) if def.name == name => Some(&mut def.methods),
                Group1Node::VariantDefine(def) if def.name == name => Some(&mut def.methods),
                _ => None,
            });

        match target {
            // 先に定義されている: その型にメゾットを追加する
            Some(type_methods) => type_methods.extend(methods),
            // まだ定義されていない: 定義されるまで予約する
            None => self.reserved_impls.entry(name).or_default().extend(methods),
        }
        Ok(())
    }

    /// `impl`の中のメゾットを解析する(フィールドは書けない)
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace`
    fn define_impl_methods(&mut self) -> Result<Vec<Group1Node>, err::ErrKind> {
        let mut methods = Vec::new();
        let mut pub_flag = false;

        loop {
            match self.next_tkn(&["name", "pub", "}"])? {
                lex::Tkn::KeyWordPub => {
                    pub_flag = true;
                }
                lex::Tkn::RBrace => break,
                lex::Tkn::Name(name) => {
                    let next = self.next_tkn_ref(&["("])?;
                    if next != lex::Tkn::LParen {
                        return crate::err_at!(self.expected_tkn("`(` (method definition)", next));
                    }
                    let mut method = self.func_node(name, pub_flag)?;
                    self.next_tkn(&[])?;
                    self.scope_counter += 1;
                    self.make_field_method_node(&mut method)?;
                    self.scope_counter -= 1;
                    // モジュールの名前を登録
                    self.registration_mod_name(&mut method)?;
                    methods.push(method);

                    // メゾットの本体を閉じる`}`の次のトークンは、次のメゾットの
                    // `name`/`pub`、または`impl`を閉じる`}`のいずれか
                    match self.current_tkn() {
                        lex::Tkn::RBrace => break,
                        lex::Tkn::Name(_) | lex::Tkn::KeyWordPub => {
                            self.back_tkn();
                        }
                        t => {
                            return crate::err_at!(
                                self.expected_tkn("name, `pub` or `}`", t.clone())
                            );
                        }
                    }
                    pub_flag = false;
                }
                t => return crate::err_at!(self.expected_tkn("name, `pub` or `}`", t)),
            }
        }

        Ok(methods)
    }
}

#[cfg(test)]
mod impl_block_test {
    use crate::compiler::{
        ast::{self, *},
        hexl_frontend::lex,
        hexl_frontend::parse,
    };

    fn parse_nodes(content: &str) -> Result<Vec<Group1Node>, ()> {
        let mut lexer = lex::Lexer::new("<test>");
        lexer.analy(&content.to_string()).unwrap();
        let tkns = lexer.gen_tkns.clone();
        let mut p = parse::Parser::new_for_test();
        p.parser_for_test(&tkns)
            .map(|nodes| nodes.to_vec())
            .map_err(|_| ())
    }

    fn method_names(methods: &[Group1Node]) -> Vec<String> {
        methods
            .iter()
            .filter_map(|m| match m {
                Group1Node::FuncDefine(f) => Some(f.name.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn impl_after_struct_adds_method() {
        let nodes =
            parse_nodes("struct A { a: int } impl A { f(a: int) {ret 1} }").expect("node is err");
        assert_eq!(nodes.len(), 1);
        let Group1Node::StructDefine(def) = &nodes[0] else {
            panic!("not struct");
        };
        assert_eq!(method_names(&def.methods), vec!["f".to_string()]);
        let Group1Node::FuncDefine(func) = &def.methods[0] else {
            panic!("not func");
        };
        assert_eq!(func.module, Some("A".to_string()));
    }

    #[test]
    fn impl_before_struct_is_reserved() {
        let nodes =
            parse_nodes("impl A { f(a: int) {ret 1} } struct A { a: int }").expect("node is err");
        assert_eq!(nodes.len(), 1);
        let Group1Node::StructDefine(def) = &nodes[0] else {
            panic!("not struct");
        };
        assert_eq!(method_names(&def.methods), vec!["f".to_string()]);
    }

    #[test]
    fn impl_for_variant() {
        let nodes =
            parse_nodes("variant V { X Y } impl V { f(a: int) {ret 1} }").expect("node is err");
        let Group1Node::VariantDefine(def) = &nodes[0] else {
            panic!("not variant");
        };
        assert_eq!(method_names(&def.methods), vec!["f".to_string()]);
    }

    #[test]
    fn impl_for_undefined_type_is_err() {
        assert!(parse_nodes("impl B { f(a: int) {ret 1} }").is_err());
    }
}
