//! 配列/ポインタへのアクセスと、ポインタ変数の加算を静的に検査する
//!
//! `gen_expr_ir`(`src/ir/builder.rs`)が式を処理するときに、
//! `check_mem_access`を一度呼び出す。範囲の判定そのものは
//! `tracking.rs`のチェッカー群と`var_tree`に記録された値を使う。

use crate::compiler::ir::checker::mem::check_list::{IndexCheckList, IsImmOrConst, IsUnsigned};

use super::*;

/// (廃止) 添字が可変な配列アクセスに挿入する実行時ガード
///
/// 添字は`const`/即値だけになり(それ以外は`check_mem_access`が`Err`を返す)、
/// `check_mem_access`は常に`None`を返す。`insert/array_guard.rs`が使う型として残している。
///
/// IR側では次の形になるように条件分岐のノードを差し込む:
/// ```text
/// if lo <= index && index < hi { arr[index] } else { arr[else_idx] }
/// ```
#[derive(Clone, Debug)]
pub(in crate::compiler::ir) struct ArrIdxGuard {
    /// 配列/ポインタの名前
    pub name: String,
    /// 有効な添字の範囲 `lo..hi`(配列は`0..len`、範囲付きポインタは`range`)
    pub lo: usize,
    pub hi: usize,
    /// ガードの対象になる(可変な)添字
    pub index: node::Expr,
    /// 範囲外だったときに使う添字(`const`/即値で、範囲内であると検証済み)
    pub else_idx: node::Expr,
}

impl IR {
    /// 配列/ポインタに関わる式を処理するときに呼び出す検査
    ///
    /// 配列の長さ(範囲付きポインタなら範囲)を測り、
    /// - 添字が`const`/即値: 静的に範囲検査してそのまま(`Ok(None)`)
    /// - 添字が`const`以外(可変な変数・引数、関数の呼び出しなど): `Err`
    pub(in crate::compiler::ir) fn check_mem_access(
        &mut self,
        expr: &node::Expr,
    ) -> Result<Option<ArrIdxGuard>, err::ErrKind> {
        match expr {
            node::Expr::RefArray {
                name,
                index,
                else_idx,
                ..
            } => {
                self.check_arr_index::<IsUnsigned>(index)?;
                return self.check_ref_array(name, index, else_idx);
            }
            node::Expr::ConnectAddr(target) => {
                if let node::Expr::Var(name) = &**target {
                    self.check_ptr_deref(name);
                }
            }
            node::Expr::PtrMember { name, .. } => self.check_ptr_deref(name),
            node::Expr::Add((l, r)) => self.check_ptr_add(expr, l, r),
            _ => {}
        }
        Ok(None)
    }

    /// `name[index else else_idx]`の検査
    ///
    /// ## Args
    /// - else_idx: `[arr idx else 0]`配列のidxが範囲外のときのidx
    fn check_ref_array(
        &mut self,
        name: &str,
        index: &node::Expr,
        _else_idx: &Option<Box<node::Expr>>,
    ) -> Result<Option<ArrIdxGuard>, err::ErrKind> {
        // 添字は`const`/即値だけ。それ以外(可変な変数・引数など)は`Err`
        // (`check_list.rs`の`IsImmOrConst`)
        self.check_arr_index::<IsImmOrConst>(index)?;

        // 配列の長さ(範囲付きポインタなら範囲)を測る。どちらでもなければ対象外
        if self.access_bounds(name).is_none() {
            return Ok(None);
        }

        // 静的に検査するだけで、IRはそのまま
        self.arr_idx_checker(name, index);
        self.range_ptr_checker(name, index);
        Ok(None)
    }

    /// 配列の長さ(`0..len`)、または範囲付きポインタの範囲を返す
    /// (どちらでもない変数は`None`)
    fn access_bounds(&self, name: &str) -> Option<(usize, usize)> {
        let ty = self.var_tree.get_ty_node(name)?;
        let size: Result<types::Size, _> = ty.try_into();
        match size {
            Ok(types::Size::Array { len, .. }) => Some((0, len)),
            Ok(types::Size::Pointer {
                range: Some(range), ..
            }) => Some(range),
            _ => None,
        }
    }

    /// 配列のindexを調べresultで返す関数
    /// Cで中の処理を変える
    fn check_arr_index<C: IndexCheckList>(&self, expr: &node::Expr) -> Result<(), err::ErrKind> {
        match expr {
            node::Expr::Number(v) => C::num_check(v),
            node::Expr::Var(name) => C::var_check(&self.var_tree, &name),
            node::Expr::Add((l, r))
            | node::Expr::Sub((l, r))
            | node::Expr::Mul((l, r))
            | node::Expr::Div((l, r)) => {
                self.check_arr_index::<C>(l)?;
                self.check_arr_index::<C>(r)?;
                Ok(())
            }
            // 条件分岐は使えない
            node::Expr::GreaterThen(..)
            | node::Expr::GreaterThenEq(..)
            | node::Expr::LessThen(..)
            | node::Expr::LessThenEq(..) => C::cond_is_invalid(&expr),
            // 関数の呼び出し・メンバーアクセスなど、値を静的に決められない式は`Err`
            t => C::unsupported_expr(t),
        }
    }

    /// 範囲付きポインタ変数の範囲を取得する
    /// (範囲指定のないポインタや、ポインタ型でない変数は`None`)
    fn ptr_range(&self, name: &str) -> Option<(usize, usize)> {
        let ty = self.var_tree.get_ty_node(name)?;
        let size: Result<types::Size, _> = ty.try_into();
        match size {
            Ok(types::Size::Pointer {
                range: Some(range), ..
            }) => Some(range),
            _ => None,
        }
    }

    /// ポインタの参照(`[name]`/`[name].member`)で、ポインタの現在値が
    /// 範囲内かを確認する
    fn check_ptr_deref(&self, name: &str) {
        let (Some(range), Some(value)) = (self.ptr_range(name), self.var_tree.get_value(name))
        else {
            return;
        };
        if value < range.0 || value >= range.1 {
            panic!(
                "範囲付きポインタ `{}` の参照が範囲外です: 値 {} (有効範囲 {}..{})",
                name, value, range.0, range.1,
            );
        }
    }

    /// ポインタ変数を含む加算の結果が、範囲内かを確認する
    fn check_ptr_add<'a>(&self, whole: &node::Expr, l: &'a node::Expr, r: &'a node::Expr) {
        let ptr_range_of = |e: &'a node::Expr| match e {
            node::Expr::Var(name) => self.ptr_range(name).map(|range| (name, range)),
            _ => None,
        };
        let Some((name, range)) = ptr_range_of(l).or_else(|| ptr_range_of(r)) else {
            return;
        };
        if self.is_const_index(whole) == false {
            return;
        }

        let value = self.eval_expr(whole);
        if value < range.0 || value >= range.1 {
            panic!(
                "範囲付きポインタ `{}` の加算結果が範囲外です: 値 {} (有効範囲 {}..{})",
                name, value, range.0, range.1,
            );
        }
    }
}
