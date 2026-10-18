mod lex;
mod parse;

pub use parse::*;

use crate::compiler::parse::node::Group1Node;

/// c言語のコードを変換
pub(in crate::compiler) fn emit_c_node(src: &str) -> Vec<Group1Node> {
    let lex = lex::Lexer::new(src);
    let tokens = lex.tokenize().unwrap();
    parse_c(tokens).unwrap().nodes
}