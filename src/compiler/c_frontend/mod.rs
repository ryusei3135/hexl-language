mod lex;
mod parse;

pub use parse::*;

use crate::compiler::ast::Group1Node;

/// c言語のコードを変換
pub(in crate::compiler) fn emit_c_node(src: &str) -> Vec<Group1Node> {
    let tokens = lex::Preprocessor::new().preprocess("<input>", src).unwrap();
    parse_c(tokens).unwrap().nodes
}
