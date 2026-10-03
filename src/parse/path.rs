use super::*;

impl Parser {
    /// モジュールのノードを作成
    /// name::mod
    pub(in crate::parse) 
    fn build_scope_node(&mut self, name: &str) -> Result<node::Expr, err::ErrKind> {
        if self.next_tkn_ref(&["{"])? == lex::Tkn::LBrace {
            self.advance_tkn().unwrap();
            let node = self.struct_init_node::<false>(name);

            return if self.next_tkn_ref(&["}"])? == lex::Tkn::RBrace {
                self.advance_tkn().unwrap();
                node
            } else {
                // }が来ていない
                panic!();
            };
        }
        if self.next_tkn_ref(&["."])? == lex::Tkn::Dot {
            return self.build_member_node(name);
        }
        // "::"がないので、何も返さない
        if self.next_tkn_ref(&["not `::`"])? != lex::Tkn::ModPathTkn {
            return Ok(self.expr_define_var(name.to_string())?);
        }

        crate::scope_node!(self, ModPathTkn, Scope, &name);
    }

    /// メゾットなどのノードを作成
    /// name.method
    #[inline(always)]
    pub(in crate::parse) 
    fn build_member_node(&mut self, name: &str) -> Result<node::Expr, err::ErrKind> {
        // "."がないので、何も返さない
        if self.next_tkn_ref(&["not `.`"])? != lex::Tkn::Dot {
            return Ok(self.expr_define_var(name.to_string())?);
        }

        // "."の次のトークンを確認するため一旦"."まで進める
        self.next_tkn(&["."])?;
        let after_dot_is_bracket = matches!(self.next_tkn_ref(&["name", "["])?, lex::Tkn::LBracket);
        // まだ"."を消費していない状態(呼び出し時点の位置)に戻す
        self.back_tkn();

        // "."の次が"["の場合、構造体の配列メンバーの要素に
        // アクセス(または代入)するノードを作成する: `name.[member index]`
        if after_dot_is_bracket {
            // "."をスキップ
            self.next_tkn(&["."])?;
            // "["をスキップ
            self.next_tkn(&["["])?;
            // expr/arr_access.rsで定義
            return self.build_member_array_node(name);
        }

        crate::scope_node!(self, Dot, Member, &name);
    }

    /// ポインタが指す構造体のメンバー/メゾットへアクセスするノードを作成する
    /// `[name].member` / `[name].method(..)`
    ///
    /// ## 呼び出し時の前提
    /// 呼び出し元で`current_tkn()`が`.`を指している状態で呼び出す
    /// (`.`自体はまだ読み飛ばしていない)
    ///
    /// ## 戻り値の状態
    /// - フィールドアクセスの場合、`current_tkn()`はメンバー名を
    ///   指した状態のまま返す(呼び出し元で`=`の有無を確認できるように)
    /// - メゾット呼び出しの場合、`call_func_expr`の仕様通り
    ///   `current_tkn()`は呼び出し式の次のトークンを指した状態で返る
    pub(in crate::parse) 
    fn ptr_member_node(&mut self, name: &str) -> Result<node::Expr, err::ErrKind> {
        let member_tkn = self.next_tkn(&["name"])?;
        let lex::Tkn::Name(member) = member_tkn.clone() else {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::ExpectedKind {
                    expected: "name",
                    found: member_tkn,
                }
            );
        };

        // メゾットの呼び出し: `[name].method(..)`
        if matches!(self.peek_tkn(), Ok(lex::Tkn::LParen)) {
            self.next_tkn(&["("])?;
            let call = self.call_func_expr(&member, true)?;
            return Ok(node::Expr::PtrMember {
                name: name.to_string(),
                target: Box::new(call),
            });
        }

        // フィールドへのアクセス: `[name].member`
        Ok(node::Expr::PtrMember {
            name: name.to_string(),
            target: Box::new(node::Expr::Var(member)),
        })
    }
}
