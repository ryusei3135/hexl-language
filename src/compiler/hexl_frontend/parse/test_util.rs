//! パーサーのテストで共通して使うヘルパー。
//!
//! `func` / `typedef` / `typedef::generic` の各テストで同じ関数が
//! 重複していたため、ここに集約している。

use crate::compiler::ast::zero_span::ZeroSpan;
use crate::compiler::ast::*;
use crate::compiler::{err, hexl_frontend::lex, hexl_frontend::parse};
use std::rc::Rc;

/// ソースを字句解析してパースする。`span`はそのまま。
pub(super) fn parse_src(src: &str) -> Result<Vec<Group1Node>, ()> {
    let mut lexer = lex::Lexer::new("<test>");
    lexer.analy(&src.to_string()).unwrap();
    let mut p = parse::Parser::new_for_test();
    p.parser_for_test(&lexer.gen_tkns.clone())
        .map(|nodes| nodes.to_vec())
        .map_err(|_| ())
}

/// `parse_src`と同じだが、すべての`span`と`Group2Info.line`を0にして返す。
pub(super) fn parse_src_zeroed(src: &str) -> Result<Vec<Group1Node>, ()> {
    parse_src(src).map(|nodes| nodes.zeroed_spans())
}

/// テストで組み立てるノードに付ける位置(`line`も`pos`も0)
pub(super) fn zero_span() -> Rc<err::Span> {
    Rc::new(err::Span::new(
        0,
        0,
        String::new(),
        &Rc::new("<test>".to_string()),
    ))
}

/// 型名だけの`TyNode`
pub(super) fn ty(name: &str) -> TyNode {
    TyNode::Ty(name.to_string())
}
