//! Parserのexpr_value関数でトークンを作成した際に
//! その次のトークンを条件次第でするめる関数を提供する
//! 
use super::*;

// `call_func_expr`は、自身で`)`を読み飛ばして呼び出し式の
// 次のトークンまで進めた状態で返ってくる(構造体初期化/配列
// リテラルが`}`を消費せず呼び出し元に委ねるのとは逆の方式)。
// そのため、値が関数呼び出し(または関数呼び出しを直接
// 包んだ`Scope`/`Member`)の場合、現在のトークンはすでに
// 呼び出し式の「次」を正しく指しており、ここでさらに
// 読み進めてはいけない
fn already_positioned_after_call(v: &Expr) -> bool {
    match &v.node {
        ExprNode::CallFunc(..) | ExprNode::Scope { .. } => true,
        ExprNode::Member { target, .. } | ExprNode::PtrMember { target, .. } => {
            matches!(target.node, ExprNode::CallFunc(..))
        }
        _ => false,
    }
}

// `self.name`のようなメンバーアクセスは、メンバー名の次のトークンまで
// 進んだ状態で返ってくる。そのため次が`)`の場合、この`)`は
// 関数呼び出しの引数の終了(`f(self.name)`)であり、すでに現在の
// トークンとして指しているので、ここで読み飛ばしてはいけない
// (読み飛ばすと`call_func_expr`が`)`を見つけられず、次の`}`などで
// `, or `)`が必要です`のエラーになる)。
// `( expr )`で囲まれた式の`)`だけは、ここで読み飛ばす
fn member_already_at_rparen(v: &Expr) -> bool {
    matches!(v.node, ExprNode::Member { .. } | ExprNode::PtrMember { .. })
}

impl Parser {
    /// 現在のトークンが数字や`)`などはスキップする
    pub fn skiped_tkn(&mut self, from_paren: bool, v: &Expr) -> Result<(), err::ErrKind> {
        if already_positioned_after_call(&v) == false {
            match self.current_tkn() {
                lex::Tkn::RParen if !from_paren && member_already_at_rparen(&v) => {}
                lex::Tkn::Name(_)
                | lex::Tkn::Number(_)
                | lex::Tkn::Float(_)
                | lex::Tkn::Str(_)
                | lex::Tkn::RParen
                | lex::Tkn::RBracket => {
                    self.next_tkn(&[])?;
                }
                // `}` は、構造体初期化(`Name { .. }`)、unsafeな共用体の初期化
                // (`Type::mem { mem: .. }`)や配列リテラル
                // (`{ .. }`)を閉じる場合にのみ、ここで読み飛ばす。
                // `self.b`のようなメンバーアクセスの直後に続く`}`は
                // 呼び出し元のブロック(関数/メゾットの本体など)を
                // 閉じるトークンなので、消費せずそのまま残す
                lex::Tkn::RBrace
                    if matches!(
                        v.node,
                        ExprNode::InitStruct { .. }
                            | ExprNode::InitVariant { .. }
                            | ExprNode::Array(..)
                    ) =>
                {
                    self.next_tkn(&[])?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}