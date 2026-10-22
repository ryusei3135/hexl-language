mod stmt;
mod tkn_mnger;

/// inlineアセンブラの`${...}`内の式を解析するAPIを提供
mod asm_expr;
mod contract;
mod err_factory;
mod expr;
mod func;
mod mut_var;
mod path;
/// プリプロセッサのノードを作成するAPIを提供
mod preproc;
/// 構造体、列挙型のノードを作成するAPIを提供
mod typedef;

use crate::compiler::{err, hexl_frontend::lex};
use stmt::*;
use crate::compiler::ast::*;

pub use stmt::Parser;
use crate::compiler::ast;

pub(in crate::compiler::hexl_frontend::parse) use parse_loop_control::*;

use crate::compiler::hexl_frontend::parse::stmt::parse_loop_control::ParseLoopControl;
use crate::err::err_pool;

use crate::compiler::{c_frontend, models::Body, ast::Group2Info};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarMutAttr {
    /// var const: t
    Const,
    /// var:t
    Invar,
    /// var mut: t
    Var,
}
