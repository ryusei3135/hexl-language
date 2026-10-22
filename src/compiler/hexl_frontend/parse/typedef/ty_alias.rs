//! `type T = BaseTy`(型エイリアス)のパース
//!
//! ```text
//! TypeAlias := KeyWordType Name "=" BaseTy
//! ```
//! - 呼び出し時、`current_tkn()`は`type`を指している
//! - 終了時は`BaseTy`のトークンを指す(`struct`が閉じの`}`を指すのと同じ規約。
//!   次のトークンへは`parse_loop`の`next_tkn`が進める)
use super::*;

impl Parser {
    pub(in crate::compiler::hexl_frontend::parse) fn type_alias_node(
        &mut self,
    ) -> Result<Group1Node, err::ErrKind> {
        // `type`の位置
        let span = self.build_err_span();

        // `T`
        let name_tkn = self.next_tkn(&["name"])?;
        let lex::Tkn::Name(name) = name_tkn.clone() else {
            return crate::err_at!(self.expected_tkn("type alias name", name_tkn));
        };

        // `=`
        let eq_tkn = self.next_tkn(&["="])?;
        if !matches!(eq_tkn, lex::Tkn::Equal) {
            return crate::err_at!(self.expected_tkn("=", eq_tkn));
        }

        // `BaseTy`
        let base_tkn = self.next_tkn(&["type name"])?;
        let lex::Tkn::Name(base) = base_tkn.clone() else {
            return crate::err_at!(self.expected_tkn("type name", base_tkn));
        };

        Ok(TypeAlias::new(name, TyNode::Ty(base), span))
    }
}
