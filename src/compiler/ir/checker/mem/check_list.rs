use super::*;
use crate::compiler::err::CompileErr;
use crate::parse::node;


pub(in crate::compiler::ir::checker::mem) 
trait IndexCheckList {
    fn num_check(val: &str) -> Result<(), err::ErrKind>;
    fn var_check(var_tree: &def_tree::VarTree, name: &str) -> Result<(), err::ErrKind>;
    /// 式で無効なもの
    fn cond_is_invalid(expr: &node::Expr) -> Result<(), err::ErrKind>;
}

/// 配列のindexが符号なし整数か調べる
pub(in crate::compiler::ir::checker::mem)
struct IsUnsigned {}

impl IndexCheckList for IsUnsigned {
    fn num_check(val: &str) -> Result<(), err::ErrKind> {
        if val.parse::<usize>().is_ok() {
            Ok(())
        } else {
            crate::GenCompileErr!(InvalidIndexUnsigned, val)
        }
    }

    fn var_check(var_tree: &def_tree::VarTree, name: &str) -> Result<(), err::ErrKind> {
        let binding: node::TyNode = var_tree.get_ty_node(name).unwrap();
        let t: types::Size = binding.try_into()?;
        if t.is_unsigned() {
            Ok(())
        } else {
            crate::GenCompileErr!(InvalidIndexUnsigned, name)
        }
    }

    fn cond_is_invalid(expr: &node::Expr) -> Result<(), err::ErrKind> {
        crate::GenCompileErrNode!(UnexpectArrIndexExprKind, expr)
    }
}

/// 配列のindexが定数か不変定数か調べる
pub(in crate::compiler::ir::checker::mem)
struct IsImmOrConst {}

impl IndexCheckList for IsImmOrConst {
    fn num_check(val: &str) -> Result<(), err::ErrKind> {
        Ok(())
    }

    fn var_check(var_tree: &def_tree::VarTree, name: &str) -> Result<(), err::ErrKind> {
        if var_tree.is_mut(&name.to_string()) != crate::compiler::parse::VarMutAttr::Const {
            crate::GenCompileErr!(InvalidIndexIsNotConst, name)
        } else {
            Ok(())
        }
    }

    fn cond_is_invalid(expr: &node::Expr) -> Result<(), err::ErrKind> {
        crate::GenCompileErrNode!(UnexpectArrIndexExprKind, expr)
    }
}