mod stmt;
mod tkn_mnger;

/// inlineアセンブラの`${...}`内の式を解析するAPIを提供
mod asm_expr;
mod err_factory;
mod expr;
mod func;
mod path;
/// プリプロセッサのノードを作成するAPIを提供
mod preproc;
/// 構造体、列挙型のノードを作成するAPIを提供
mod typedef;
pub mod node;

use crate::{err, lex};
use stmt::*;

pub use stmt::Parser;
