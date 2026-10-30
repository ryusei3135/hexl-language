//! Cのパーサ: `lex/preproc.rs`が出力したトークン列から、`node`のノード列を作る。
//!
//! ```ignore
//! let tokens = Preprocessor::new().preprocess("a.c", src)?;
//! let out = parse::parse_c(tokens)?;   // out.nodes: Vec<Group1Node>
//! ```
//!
//! 流れは `トークン -> (parser/) CAST -> (lower.rs) node`。
//! `node`で表現できないものは`ParseErrorKind::Unsupported`になる。一覧はNOTE.md。

mod cast;
mod error;
mod lower;
mod parser;
mod prelude;
#[cfg(test)]
mod tests;

use std::rc::Rc;

pub(in crate::compiler::c_frontend) use error::ParseError;
// テストでだけ種類を見る
#[cfg(test)]
use error::ParseErrorKind;

use prelude::{Group1Node, TokenInfo};

/// パース結果
#[derive(Debug, Clone)]
pub(in crate::compiler::c_frontend) struct ParseOutput {
    /// 関数・構造体・共用体・列挙型の定義(ソースに現れた順)
    pub nodes: Vec<Group1Node>,
    /// 読み飛ばしたもの・近似したものの一覧
    pub warnings: Vec<String>,
}

/// プリプロセス済みのトークン列(`Preprocessor::preprocess`の戻り値)をパースする
pub(in crate::compiler::c_frontend) fn parse_c(
    tokens: Vec<TokenInfo>,
    self_file_name: &Rc<String>,
) -> Result<ParseOutput, ParseError> {
    let (items, mut warnings) = parser::Parser::new(tokens).parse_unit()?;
    let (nodes, w2) = lower::Lowerer::new(
        self_file_name,
    )
    .lower_unit(items)?;
    warnings.extend(w2);
    Ok(ParseOutput { nodes, warnings })
}
