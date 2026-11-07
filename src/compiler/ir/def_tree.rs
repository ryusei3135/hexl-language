use std::rc::Rc;

use crate::compiler::{
    err::compile::{CompileErr, CompileErrKind},
    hexl_frontend::parse,
};
use crate::err::Span;

use super::types::*;
use super::*;
use crate::compiler::ast::{ArgsNode, FuncDefine, StructDefine, TyNode, VariantDefine};

mod func_tree;
mod type_tree;
mod var_tree;

pub use func_tree::*;
pub use type_tree::*;
pub use var_tree::*;
