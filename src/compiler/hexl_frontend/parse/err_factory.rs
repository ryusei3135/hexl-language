pub(in crate::compiler::hexl_frontend::parse) mod cond_syn;
pub(in crate::compiler::hexl_frontend::parse) mod typedef;

pub(in crate::compiler::hexl_frontend::parse) mod fn_err;

pub(in crate::compiler::hexl_frontend::parse) mod common_err;
pub(in crate::compiler::hexl_frontend::parse) mod contract_err;
pub(in crate::compiler::hexl_frontend::parse) mod expr_err;
pub(in crate::compiler::hexl_frontend::parse) mod preproc_err;
pub(in crate::compiler::hexl_frontend::parse) mod stmt_err;
pub(in crate::compiler::hexl_frontend::parse) mod tkn_err;
/// エラーの発生元を`println!`する仕組み(`err_at!`など)
pub mod trace;

use std::rc::Rc;

use super::*;
