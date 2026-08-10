
mod tkn_mnger;
mod stmt;

mod func;
mod expr;
/// プリプロセッサのノードを作成するAPIを提供
mod preproc;
mod path;
/// 構造体、列挙型のノードを作成するAPIを提供
mod typedef;
/// inlineアセンブラの`${...}`内の式を解析するAPIを提供
mod asm_expr;
mod err_factory;

use crate::{
    err,
    lex,
    node,
};
use tkn_mnger::*;
use stmt::*;

pub use stmt::Parser;
