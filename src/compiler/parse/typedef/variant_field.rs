use crate::compiler::node::{Field, VariantField, VariantFieldKind};

use super::*;

/// メゾットかを調べる時に使う
const IS_METHOD: bool = true;
const IS_FIELD: bool = false;

type IsVariantMethod = bool;

impl Parser {
    /// 共用体のフィールドを作成する
    /// 呼び出し時は current_tkn() がメンバーの名前。
    /// 終了時は、そのメンバーの最後のトークン(型なしなら名前、
    /// `Mem(ty)`なら`)`、メゾットなら本体を閉じる`}`)を指す
    pub(in crate::compiler::parse::typedef)
    fn make_variant_field(
        &mut self,
        field_name: String,
        pub_flag: bool,
    ) -> Result<VariantFieldKind, err::ErrKind> {
        match self.peek_tkn()? {
            lex::Tkn::LParen => {
                // 現在のトークンを`(`にする
                self.advance_tkn();
                if self.check_this_is_fn() == IS_METHOD {
                    // メゾットとして処理
                    let method = self.make_variant_method(field_name, pub_flag)?;
                    Ok(method.wrap_variant_field_kind())
                } else {
                    // 型を持つメンバーとして処理
                    let ty = self.make_ty_variant_field()?;
                    Ok(VariantField::new(field_name, ty).wrap_variant_field_kind())
                }
            }
            // 型なしのメンバー: 次に続くのは別のメンバー・`,`・`}`
            lex::Tkn::Name(..)
            | lex::Tkn::KeyWordPub
            | lex::Tkn::Comma
            | lex::Tkn::RBrace => Ok(VariantField::typeless_new(field_name).wrap_variant_field_kind()),
            t => crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::UnexpectedTkn {
                    found: t,
                    expected: lex::Tkn::RBrace,
                    context: lex::Tkn::KeyWordVariant
                }
            ),
        }
    }

    /// フィールドの中の型を作成する。
    /// `Mem(ty)`の`ty`を作成する。
    /// 呼び出し時は current_tkn() が`(`、終了時は`)`を指す
    /// ```text
    /// variant A {Mem(ty)}
    /// ```
    fn make_ty_variant_field(&mut self) -> Result<node::TyNode, err::ErrKind> {
        #[cfg(test)]
        if self.current_tkn() != &lex::Tkn::LParen {
            panic!("`(`じゃない");
        }

        // `ty_node_after_delim`は、区切り(`(`)の次から型を読み、
        // 型の次のトークンを指して終了する
        let ty = self.ty_node_after_delim()?;
        if self.current_tkn() != &lex::Tkn::RParen {
            self.variant_in_unexpect_tkn(lex::Tkn::RParen)?;
        }
        Ok(ty)
    }

    /// 処理中のフィールドがメゾットか調べる関数。
    /// 呼び出し時は current_tkn() が`(`
    /// `Mem1(int)`ならfalse
    /// `Mem2(a: ty)` / `Mem3()` / `Mem4(mut a: ty)`ならtrueを返す
    /// ```text
    /// variant A {
    ///     Mem1(int)
    ///     Mem2(a: ty)
    /// }
    /// ```
    #[inline(always)]
    fn check_this_is_fn(&self) -> IsVariantMethod {
        // `(`の次が`)`/`mut`なら引数の定義
        if let Ok(lex::Tkn::RParen | lex::Tkn::KeyWordMut) = self.peek_tkn() {
            return IS_METHOD;
        }
        // `(`の2つ先が`:`なら`name: ty`という引数の定義
        match self.peek2_tkn() {
            Some(lex::Tkn::Colon) => IS_METHOD,
            _ => IS_FIELD,
        }
    }

    /// 共用体のメゾットのノードを作成する。
    /// 呼び出し時は current_tkn() が`(`、
    /// 終了時はメゾットの本体を閉じる`}`を指す
    fn make_variant_method(
        &mut self,
        method_name: String,
        pub_flag: bool,
    ) -> Result<node::Group1Node, err::ErrKind> {
        #[cfg(test)]
        if self.current_tkn() != &lex::Tkn::LParen {
            panic!("`(`じゃない");
        }
        // `(`の前まで戻す(`func_node`が`(`を読む)
        self.back_tkn();

        // 終了時は`{`を指す
        let mut method = self.func_node(method_name, pub_flag)?;
        self.next_tkn(&[])?;
        self.scope_counter += 1;

        self.make_field_method_node(&mut method)?;
        self.scope_counter -= 1;
        // モジュールの名前を登録
        self.registration_mod_name(&mut method);
        // `make_field_method_node`は`}`の次のトークンまで進めるので、
        // 呼び出し元のループの`next_tkn`で次のメンバーを読めるように戻す
        self.back_tkn();
        Ok(method)
    }
}

#[cfg(test)]
mod variant_node_test {
    use crate::compiler::{
        lex,
        node::{self, *},
        parse,
    };

    fn gen_variant(content: &str) -> node::VariantDefine {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&content.to_string()).unwrap();
        let tkns = lexer.gen_tkns.clone();
        let mut p = parse::Parser::new();
        let nodes = p.parser(&tkns).expect("node is err");
        let node::Group1Node::VariantDefine(variant_node) = nodes[0].clone() else {
            panic!("not variant");
        };
        variant_node
    }

    #[test]
    fn check_typeless_variant_node() {
        assert_eq!(
            gen_variant("variant A {A}"),
            VariantDefine {
                name: "A".to_string(),
                fields: vec![node::VariantField::typeless_new("A".to_string())],
                methods: Vec::new(),
            }
        );
    }

    #[test]
    fn check_variant_with_method() {
        let variant_node = gen_variant("variant A {A func(a: int) {ret 1}}");
        assert_eq!(
            variant_node.fields,
            vec![node::VariantField::typeless_new("A".to_string())]
        );
        assert_eq!(variant_node.methods.len(), 1);
        let node::Group1Node::FuncDefine(func) = &variant_node.methods[0] else {
            panic!("not func");
        };
        assert_eq!(func.name, "func");
        assert_eq!(func.module, Some("A".to_string()));
    }

    #[test]
    fn check_variant_with_ty_field() {
        let variant_node = gen_variant("variant A {B(int) C}");
        assert_eq!(variant_node.fields.len(), 2);
        assert_eq!(variant_node.fields[0].name, "B");
        assert_eq!(
            variant_node.fields[0].ty,
            Some(node::TyNode::Ty("int".to_string()))
        );
        assert_eq!(variant_node.fields[1].ty, None);
    }
}
