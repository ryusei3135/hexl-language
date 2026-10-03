//! self.expr_valueのmatchの中から呼び出すAPIを提供

use super::{
    *,
    arr_access::*,
};

impl Parser {
    /// 呼び出しもとで、トークン`lex::Tkn::Name(..)`が
    /// あった場合呼び出される、関数やモジュールの指定メンバー
    /// にアクセスするノードを作成する
    pub(super) 
    fn gen_name_node<const T: bool>(
        &mut self,
        name: String,
        init_struct: bool,
    ) -> Result<node::Expr, err::ErrKind> {
        // `init_struct`が`false`の場合構造体を初期化してはいけないので、変数を返す
        if init_struct == false {
            return Ok(node::Expr::Var(name));
        }
        let node = match self.next_tkn_ref(&[".", "(", "`", "::"])? {
            lex::Tkn::Dot => {
                let n = self.build_scope_node(&name);
                return n;
            }
            // 関数を呼びだすノードを作成
            lex::Tkn::LParen => {
                self.advance_tkn().unwrap();
                self.call_func_expr(&name, true)?
            }
            // ジェネリクス関数を呼びだすノードを作成: `func<int>(..)`
            // (比較の`a < b`と区別するため、`name`が定義済みの
            // ジェネリクス関数で、`<..>(`の形のときだけ)
            lex::Tkn::LAngleBracket if self.is_generic_call(&name, self.idx + 1) => {
                // `<`まで進める
                self.advance_tkn().unwrap();
                self.generic_call_expr(&name, true)?
            }
            // 構造体の初期化ノードを作成する
            lex::Tkn::LBrace => {
                // "{"から始まらないといけないので、次に進める
                self.next_tkn(&["{"])?;
                return self.struct_init_node::<T>(&name);
            }
            // 列挙型のメンバへのアクセス: `Name::Mem`
            lex::Tkn::ModPathTkn => {
                self.next_tkn(&["name"])?;
                let lex::Tkn::Name(mem_name) = self.next_tkn(&["name"])?.clone() else {
                    panic!();
                };
                if matches!(self.next_tkn_ref(&[])?, lex::Tkn::LParen) {
                    self.next_tkn(&["("])?;
                    node::Expr::Scope {
                        scope: vec![name],
                        target: Box::new(self.call_func_expr(&mem_name, init_struct)?),
                    }
                } else {
                    node::Expr::EnumVariant {
                        name,
                        variant: mem_name,
                    }
                }
            }
            _ => node::Expr::Var(name),
        };
        Ok(node)
    }

    /// 配列リテラルのノードを作成する
    /// これは配列を初期化するノード
    pub(super)
    fn make_array_node(&mut self) -> Result<node::Expr, err::ErrKind> {
        let mut items = Vec::<node::Expr>::new();

        if self.next_tkn_ref(&["not `}`"])? != lex::Tkn::RBrace {
            loop {
                // 初期化構造体のノードを作成可能
                items.push(self.expr_cmp(true)?);

                match self.current_tkn() {
                    lex::Tkn::Comma => continue,
                    lex::Tkn::RBrace => break,
                    t => panic!("{:?}", t),
                }
            }
        }
        Ok(node::Expr::Array(items))
    }
}
