//! self.expr_valueのmatchの中から呼び出すAPIを提供

use super::*;


impl Parser {
    /// 変数のアドレス取得などのノードを作成
    /// 呼び出し元では、lex::Tkn::LBracket
    pub(super) fn get_var_addr_node(
        &mut self
    ) -> Result<node::Expr, err::ErrKind> {
        let lex::Tkn::Name(name) = self.next_tkn_ref(vec!["name"])? else {panic!()};
        let result = self.expr_add(true)?;
        let node = match self.current_tkn().clone() {
            lex::Tkn::LBracket => {
                dbg!(self.current_tkn());
                Err(err::ErrKind::UnexpectedToken)?
            }
            // nameの次に、数字が来た場合、それは配列にアクセスする
            lex::Tkn::Number(index) => {
                let _ = self.next_tkn(vec!["]"])?;
                node::Expr::InsertArr {
                    name,
                    dst: Box::new(result),
                    index: index.parse::<usize>().unwrap(),
                }
            }
            _ => {
                node::Expr::GetAddress(Box::new(result))
            }
        };
        Ok(node)
    }

    /// 呼び出しもとで、トークン`lex::Tkn::Name(..)`が
    /// あった場合呼び出される、関数やモジュールの指定メンバー
    /// にアクセスするノードを作成する
    pub(super) fn gen_name_node(
        &mut self,
        name: String,
        init_struct: bool
    ) -> Result<node::Expr, err::ErrKind> {
        // `init_struct`が`false`の場合構造体を初期化してはいけないので、変数を返す 
        if !init_struct {
            return Ok(node::Expr::Var(name));
        }
        let node = match self.next_tkn_ref(vec![".", "(", "`", "::"])? {
            lex::Tkn::Dot => {
                let n = self.build_scope_node(&name);
                println!("value_api gen_name_node ff {:?} {:?}", self.current_tkn(), self.peek_tkn());
                return n;
            }
            // 関数を呼びだすノードを作成
            lex::Tkn::LParen => {
                self.advance_tkn().unwrap();
                self.call_func_expr(&name, true)?
            }
            // 構造体の初期化ノードを作成する
            lex::Tkn::LBrace => {
                // "{"から始まらないといけないので、次に進める
                self.next_tkn(vec!["{"])?;
                return self.struct_init_node(&name);
            }
            // 列挙型のメンバへのアクセス: `Name::Mem`
            lex::Tkn::ModPathTkn => {
                self.next_tkn(vec!["name"])?;
                let lex::Tkn::Name(mem_name) = self.next_tkn(vec!["name"])?.clone() else {panic!();};
                if matches!(self.next_tkn_ref(vec![])?, lex::Tkn::LParen) {
                    self.next_tkn(vec!["("])?;
                    node::Expr::Scope {
                        scope: vec![name],
                        target: Box::new(self.call_func_expr(&mem_name, init_struct)?),
                    }
                } else {
                    node::Expr::EnumVariant { name, variant: mem_name }
                }
            }
            _ => node::Expr::Var(name)
        };
        Ok(node)
    }

    /// 配列リテラルのノードを作成する
    /// これは配列を初期化するノード
    pub(super) fn make_array_node(
        &mut self
    ) -> Result<node::Expr, err::ErrKind> {
        let mut items = Vec::<node::Expr>::new();

        if !matches!(self.next_tkn_ref(vec!["not `}`"])?, lex::Tkn::RBrace) {
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
