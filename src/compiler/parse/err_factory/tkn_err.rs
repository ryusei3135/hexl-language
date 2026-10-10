//! トークンの読み取り(`tkn_mnger.rs`)で発生するエラー

use super::*;

impl Parser {
    /// 読み取ろうとしたトークンが存在しない(EOF)
    ///
    /// ## Args
    /// - expected 期待していたトークンの説明(不明なら空)
    pub(in crate::compiler::parse)
    fn tkn_is_eof<T>(&self, expected: Vec<&'static str>) -> Result<T, err::ErrKind> {
        crate::err_generated_by!();
        crate::syntax_err!(
            self.build_err_span(),
            err::SyntaxErrKind::TknIsEof { expected }
        )
    }
}
