use super::*;
use crate::compiler::err::CompileErr;
use crate::parse::node;

pub(in crate::compiler::ir::checker::mem) trait IndexCheckList {
    fn num_check(val: &str) -> Result<(), err::ErrKind>;
    fn var_check(var_tree: &def_tree::VarTree, name: &str) -> Result<(), err::ErrKind>;
    /// 式で無効なもの
    fn cond_is_invalid(expr: &node::Expr) -> Result<(), err::ErrKind>;
    /// indexに使えない種類の式(関数の呼び出し・メンバーアクセスなど)。
    /// 値を静的に決められないので、必ず`Err`を返す
    fn unsupported_expr(expr: &node::Expr) -> Result<(), err::ErrKind> {
        crate::GenCompileErrNode!(UnexpectArrIndexExprKind, expr)
    }
}

/// 配列のindexが符号なし整数か調べる
pub(in crate::compiler::ir::checker::mem) struct IsUnsigned {}

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

/// 配列のindexが`const`か即値か調べる
///
/// `const`以外の変数(`mut`/不変の変数・引数)や、値を静的に決められない式は`Err`を返す
pub(in crate::compiler::ir::checker::mem) struct IsImmOrConst {}

impl IndexCheckList for IsImmOrConst {
    fn num_check(val: &str) -> Result<(), err::ErrKind> {
        Ok(())
    }

    fn var_check(var_tree: &def_tree::VarTree, name: &str) -> Result<(), err::ErrKind> {
        // 引数など、`const`でない変数は`Err`
        if var_tree.get_ty_node(name).is_none()
            || var_tree.is_mut(&name.to_string()) != crate::compiler::parse::VarMutAttr::Const
        {
            crate::GenCompileErr!(InvalidIndexIsNotConst, name)
        } else {
            Ok(())
        }
    }

    fn cond_is_invalid(expr: &node::Expr) -> Result<(), err::ErrKind> {
        crate::GenCompileErrNode!(UnexpectArrIndexExprKind, expr)
    }
}
