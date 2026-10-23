//! このディレクトリが外部に依存する型・パスをここに集約する。
//! プロジェクトのモジュール構成が違う場合は、このファイルの `use` だけ直せばよい。

/// `node.zip` の中身(Expr / Group1Node / TyNode ...)
pub(super) use crate::compiler::ast::*;
/// `lex/lexer.rs` のトークン
pub(super) use crate::compiler::c_frontend::lex::{Token, TokenInfo};
/// 変数の可変/不変(`Invar` と `Var` だけを使う)
pub(super) use crate::compiler::hexl_frontend::parse::VarMutAttr;
/// `Match` / `Loop` の本体の型。`Vec<Group2Info>` として組み立てる(NOTE.md 参照)
pub(super) use crate::compiler::models::Body;
