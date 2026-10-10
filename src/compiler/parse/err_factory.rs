pub(in crate::compiler::parse) mod cond_syn;
pub(in crate::compiler::parse) mod typedef;

pub(in crate::compiler::parse) mod fn_err;

pub(in crate::compiler::parse) mod contract_err;
pub(in crate::compiler::parse) mod expr_err;
pub(in crate::compiler::parse) mod preproc_err;
pub(in crate::compiler::parse) mod stmt_err;
pub(in crate::compiler::parse) mod tkn_err;
/// エラーの発生元を`println!`する仕組み(`err_at!`など)
pub mod trace;

use super::*;
