//! self.expr_valueのmatchの中から呼び出すAPIを提供

use super::*;

impl Parser {
    /// 呼び出しもとで、トークン`lex::Tkn::Name(..)`が
    /// あった場合呼び出される、関数やモジュールの指定メンバー
    /// にアクセスするノードを作成する
    pub(super) fn gen_name_node<const T: bool>(
        &mut self,
        name: String,
        init_struct: bool,
    ) -> Result<node::Expr, err::ErrKind> {
        // 名前の位置
        let span = std::rc::Rc::new(self.build_err_span());
        // `init_struct`が`false`の場合構造体を初期化してはいけないので、変数を返す
        if init_struct == false {
            return Ok(node::Expr::new(node::ExprNode::Var(name), span));
        }
        let node = match self.next_tkn_ref(&[".", "(", "`", "::"])? {
            lex::Tkn::Dot => {
                let n = self.build_scope_node(&name);
                return n;
            }
            // 関数を呼びだすノードを作成
            lex::Tkn::LParen => {
                self.advance_tkn_checked()?;
                self.call_func_expr(&name, true)?
            }
            // ジェネリクス関数を呼びだすノードを作成: `func<int>(..)`
            // (比較の`a < b`と区別するため、`name`が定義済みの
            // ジェネリクス関数で、`<..>(`の形のときだけ)
            lex::Tkn::LAngleBracket if self.is_generic_call(&name, self.idx + 1) => {
                // `<`まで進める
                self.advance_tkn_checked()?;
                self.generic_call_expr(&name, true)?
            }
            // ジェネリクスな構造体/バリアントの初期化・メゾットの呼び出し:
            // `Name<int> { .. }` / `Name<int>::Mem`
            lex::Tkn::LAngleBracket if self.is_generic_type_expr(&name, self.idx + 1) => {
                // `<`まで進める
                self.advance_tkn_checked()?;
                self.generic_type_expr::<T>(&name, init_struct)?
            }
            // 構造体の初期化ノードを作成する
            lex::Tkn::LBrace => {
                // "{"から始まらないといけないので、次に進める
                self.next_tkn(&["{"])?;
                return self.struct_init_node::<T>(&name);
            }
            // 列挙型のメンバへのアクセス: `Name::Mem`
            lex::Tkn::ModPathTkn => self.scope_member_node(name, init_struct)?,
            _ => node::Expr::new(node::ExprNode::Var(name), span),
        };
        Ok(node)
    }

    /// `Name::Mem` / `Name::func(..)`のノードを作成する。
    /// 呼び出し時の`current_tkn()`は、`::`の1つ前のトークン
    /// (型の名前。ジェネリクスな型の場合は`>`)
    pub(in crate::compiler::parse) fn scope_member_node(
        &mut self,
        name: String,
        init_struct: bool,
    ) -> Result<node::Expr, err::ErrKind> {
        // 型の名前の位置
        let span = std::rc::Rc::new(self.build_err_span());
        self.next_tkn(&["name"])?;
        let found = self.next_tkn(&["name"])?;
        let lex::Tkn::Name(mem_name) = found.clone() else {
            return crate::err_at!(self.expected_tkn("name", found));
        };
        if self.is_variant_field(&name, &mem_name) {
            // unsafeな共用体の初期化: `Type::mem { mem: expr }`
            // (`Type::Mem(expr)`はタグ付き共用体用なので、`{`が続くときだけここに入る)
            if self.is_unsafe_variant(&name) && matches!(self.next_tkn_ref(&[])?, lex::Tkn::LBrace)
            {
                self.next_tkn(&["{"])?;
                return self.unsafe_variant_init_node(name, &mem_name);
            }
            return self.variant_init_node::<false>(&name);
        }
        if matches!(self.next_tkn_ref(&[])?, lex::Tkn::LParen) {
            self.next_tkn(&["("])?;
            Ok(node::Expr::new(
                node::ExprNode::Scope {
                    scope: vec![name],
                    target: Box::new(self.call_func_expr(&mem_name, init_struct)?),
                },
                span,
            ))
        } else {
            Ok(node::Expr::new(
                node::ExprNode::EnumVariant {
                    name,
                    variant: mem_name,
                },
                span,
            ))
        }
    }

    /// 渡されたモジュールの名前とフィールドの名前がすでに出現している場合、
    /// これはバリアントなのでtrueを返す
    pub(in crate::compiler::parse) fn is_variant_field(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> bool {
        self.gen_nodes
            .iter()
            .chain(self.pending_types.iter())
            .any(|node| {
                let node::Group1Node::VariantDefine(def) = node else {
                    return false;
                };
                let is_matching_type = def.name == type_name
                    || def
                        .name
                        .strip_prefix(type_name)
                        .is_some_and(|suffix| suffix.starts_with('$'));
                is_matching_type && def.find_field(field_name).is_some()
            })
    }

    /// `type_name`がunsafeな共用体(タグを持たない)として定義済みか調べる。
    /// `is_variant_field`と同様に、ジェネリクスで増えた`Name$int`も対象にする
    fn is_unsafe_variant(&self, type_name: &str) -> bool {
        self.gen_nodes
            .iter()
            .chain(self.pending_types.iter())
            .any(|node| {
                let node::Group1Node::VariantDefine(def) = node else {
                    return false;
                };
                let is_matching_type = def.name == type_name
                    || def
                        .name
                        .strip_prefix(type_name)
                        .is_some_and(|suffix| suffix.starts_with('$'));
                is_matching_type && !def.is_tagged()
            })
    }

    /// unsafeな共用体の初期化ノードを作成する: `Type::mem { mem: expr }`
    /// 呼び出し時の`current_tkn()`は`{`。
    /// `{ .. }`の中身は構造体の初期化と同じ書き方なので`struct_init_node`で読み、
    /// `InitVariant`に変換する。フィールドはちょうど1つで、
    /// `::`の後ろの名前(`mem_name`)と一致していなければならない
    fn unsafe_variant_init_node(
        &mut self,
        name: String,
        mem_name: &str,
    ) -> Result<node::Expr, err::ErrKind> {
        let init = self.struct_init_node::<false>(&name)?;
        let span = init.span.clone();
        let node::ExprNode::InitStruct {
            is_self, fields, ..
        } = init.node
        else {
            return crate::err_at!(self.internal_state_err("struct init"));
        };
        if fields.len() != 1 || !fields.contains_key(mem_name) {
            let found = self.current_tkn().clone();
            return crate::err_at!(
                self.expected_tkn("exactly one field matching the member", found)
            );
        }
        Ok(node::Expr::new(
            node::ExprNode::InitVariant {
                is_self,
                name,
                fields,
            },
            span,
        ))
    }

    /// 配列リテラルのノードを作成する
    /// これは配列を初期化するノード
    pub(super) fn make_array_node(&mut self) -> Result<node::Expr, err::ErrKind> {
        // `{`の位置
        let span = std::rc::Rc::new(self.build_err_span());
        let mut items = Vec::<node::Expr>::new();

        if self.next_tkn_ref(&["not `}`"])? != lex::Tkn::RBrace {
            loop {
                // 初期化構造体のノードを作成可能
                items.push(self.expr_or(true)?);

                match self.current_tkn() {
                    lex::Tkn::Comma => continue,
                    lex::Tkn::RBrace => break,
                    t => return crate::err_at!(self.expected_tkn(", or `}`", t.clone())),
                }
            }
        }
        Ok(node::Expr::new(node::ExprNode::Array(items), span))
    }
}
