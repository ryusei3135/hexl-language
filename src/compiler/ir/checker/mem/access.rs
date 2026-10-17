//! 配列/ポインタへのアクセスと、ポインタ変数の加算を静的に検査する
//!
//! `gen_expr_ir`(`src/ir/builder.rs`)が式を処理するときに、
//! `check_mem_access`を一度呼び出す。範囲の判定そのものは
//! `tracking.rs`のチェッカー群と`var_tree`に記録された値を使う。

use super::*;
use crate::compiler::err::compile::CompileErr;
use crate::compiler::ir::checker::mem::check_list::{IndexCheckList, IsImmOrConst, IsUnsigned};

/// 添字が可変な配列アクセスに挿入する実行時ガード
///
/// 添字が`const`/即値のときは静的に検査して`None`を返し、
/// 可変な添字(`else`の添字が範囲内の`const`/即値)のときは`check_ref_array`がこのガードを生成する。
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
    /// - 添字が可変で範囲内と証明できない: `else`の添字を検証して実行時ガードを返す
    ///   (`else`が無い、または長さ/範囲が分からない場合は`Err`)
    ///
    /// 添字付きのポインタ(`[ptr index else else_idx].member`)も、配列と同じ検査をする
    pub(in crate::compiler::ir) fn check_mem_access<M: ResultMod>(
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
                return M::handle(self.check_ref_array::<M>(name, index, else_idx));
            }
            node::Expr::ConnectAddr(target) => {
                if let node::Expr::Var(name) = &**target {
                    M::handle(self.check_ptr_deref::<M>(name))?;
                }
            }
            // `[ptr index else else_idx].member`: 配列(`RefArray`)と同じ検査/ガード
            node::Expr::PtrMember {
                name,
                index: Some(index),
                else_idx,
                ..
            } => {
                self.check_arr_index::<IsUnsigned>(index)?;
                return M::handle(self.check_ref_array::<M>(name, index, else_idx));
            }
            // `[ptr].member`: ポインタの現在値が範囲内かを確認する
            node::Expr::PtrMember { name, .. } => M::handle(self.check_ptr_deref::<M>(name))?,
            node::Expr::Add((l, r)) => M::handle(self.check_ptr_add::<M>(expr, l, r))?,
            _ => {}
        }
        Ok(None)
    }

    /// `name[index else else_idx]`(配列)、`[name index else else_idx].member`
    /// (範囲付きポインタ)の検査
    ///
    /// ## Args
    /// - else_idx: `[arr idx else 0]`配列/ポインタのidxが範囲外のときのidx
    fn check_ref_array<M: ResultMod>(
        &mut self,
        name: &str,
        index: &node::Expr,
        else_idx: &Option<Box<node::Expr>>,
    ) -> Result<Option<ArrIdxGuard>, err::ErrKind> {
        // 添字は`const`/即値だけ。それ以外(可変な変数・引数など)は`Err`
        // (`check_list.rs`の`IsImmOrConst`)
        match self.check_arr_index::<IsImmOrConst>(index) {
            // 添字が可変なときは、実行時ガード(`ArrIdxGuard`)を生成して返す
            Err(e) if Self::is_not_const_index_err(&e) => {
                // 長さ/範囲が分からない変数、`else`の添字がない場合はガードを作れない
                let range = match self.access_bounds::<M>(name) {
                    Ok(range) => {
                        range
                    }
                    Err(e) => {
                        panic!("{:?} {:?}", e, name);
                    }
                };

                // 添字の取りうる範囲が静的に分かり、`lo..hi`に収まると証明できるなら
                // ガードは不要。通常の静的検査(下)にそのまま進む
                let proven = matches!(
                    self.index_interval(index),
                    Some((min, max)) if min >= range.0 && max < range.1
                );
                if proven {
                    // fall through
                } else {
                    let Some(else_expr) = else_idx else {
                        return Err(e);
                    };

                    // `else`の添字も`const`/即値だけで、範囲内であることを検証する
                    self.check_arr_index::<IsImmOrConst>(else_expr)?;
                    if self.is_const_index(else_expr) {
                        let value = self.eval_expr(else_expr);
                        if value < range.0 || value >= range.1 {
                            M::handle(crate::GenCompileErr2!(
                                CompileErr::PointerRangeLengthMismatch {
                                    range: range,
                                    value_len: value,
                                }
                            ))?;
                        }
                    }

                    return Ok(Some(ArrIdxGuard {
                        name: name.to_string(),
                        lo: range.0,
                        hi: range.1,
                        index: index.clone(),
                        else_idx: (**else_expr).clone(),
                    }));
                }
            }
            t => t?,
        }

        // 配列の長さ(範囲付きポインタなら範囲)を測る。どちらでもなければ対象外
        if self.access_bounds::<M>(name).is_err() {
            return Ok(None);
        }

        // 静的に検査するだけで、IRはそのまま
        self.arr_idx_checker(name, index);
        self.range_ptr_checker(name, index);
        Ok(None)
    }

    /// 添字式の取りうる値の閉区間`(min, max)`を静的に求める
    /// (`var_tree`に値が記録されていない変数や、求められない式は`None`)
    ///
    /// 区間が`lo..hi`に収まれば、可変な添字でも実行時ガードなしで通せる。
    fn index_interval(&self, expr: &node::Expr) -> Option<(usize, usize)> {
        match expr {
            node::Expr::Number(_) => {
                if self.is_const_index(expr) {
                    let v = self.eval_expr(expr);
                    Some((v, v))
                } else {
                    None
                }
            }
            node::Expr::Var(name) => self.var_tree.get_value(name).map(|v| (v, v)),
            node::Expr::Add((l, r)) => {
                let (a, b) = (self.index_interval(l)?, self.index_interval(r)?);
                Some((a.0.checked_add(b.0)?, a.1.checked_add(b.1)?))
            }
            node::Expr::Sub((l, r)) => {
                let (a, b) = (self.index_interval(l)?, self.index_interval(r)?);
                // 負になりうるなら証明できない
                Some((a.0.checked_sub(b.1)?, a.1.checked_sub(b.0)?))
            }
            node::Expr::Mul((l, r)) => {
                let (a, b) = (self.index_interval(l)?, self.index_interval(r)?);
                Some((a.0.checked_mul(b.0)?, a.1.checked_mul(b.1)?))
            }
            node::Expr::Div((l, r)) => {
                let (a, b) = (self.index_interval(l)?, self.index_interval(r)?);
                if b.0 == 0 {
                    return None;
                }
                Some((a.0 / b.1, a.1 / b.0))
            }
            _ => None,
        }
    }

    /// 配列の長さ(`0..len`)、または範囲付きポインタの範囲を返す
    /// (どちらでもない変数は`None`)
    fn access_bounds<M: ResultMod>(&self, name: &str) -> Result<(usize, usize), types::Size> {
        let ty = self.var_tree.get_ty_node(name).unwrap();
        let size: Result<types::Size, _> = ty.clone().try_into();
        println!("{:?} {:?}", size, ty);
        match size {
            Ok(types::Size::Array { len, .. }) => Ok((0, len)),
            Ok(types::Size::Pointer {
                range, ..
            }) => Ok(range.unwrap_or((0, 0))),
            Ok(t) => Err(t),
            Err(e) => panic!("{:?}", e),
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
    fn check_ptr_deref<M: ResultMod>(&self, name: &str) -> Result<(), err::ErrKind> {
        let (Some(range), Some(value)) = (self.ptr_range(name), self.var_tree.get_value(name))
        else {
            return Ok(());
        };
        if value < range.0 || value >= range.1 {
            M::handle(crate::GenCompileErr2!(
                CompileErr::PointerRangeLengthMismatch {
                    range: range,
                    value_len: value,
                }
            ))
        } else {
            Ok(())
        }
    }

    /// ポインタ変数を含む加算の結果が、範囲内かを確認する
    fn check_ptr_add<'a, M: ResultMod>(
        &self,
        whole: &node::Expr,
        l: &'a node::Expr,
        r: &'a node::Expr,
    ) -> Result<(), err::ErrKind> {
        let ptr_range_of = |e: &'a node::Expr| match e {
            node::Expr::Var(name) => self.ptr_range(name).map(|range| (name, range)),
            _ => None,
        };
        let Some((name, range)) = ptr_range_of(l).or_else(|| ptr_range_of(r)) else {
            return Ok(());
        };
        if self.is_const_index(whole) == false {
            return Ok(());
        }

        let value = self.eval_expr(whole);
        if value < range.0 || value >= range.1 {
            M::handle(crate::GenCompileErr2!(
                CompileErr::PointerRangeLengthMismatch {
                    range: range,
                    value_len: value,
                }
            ))
        } else {
            Ok(())
        }
    }

    /// 「添字が`const`/即値ではない」エラーかを判定する
    fn is_not_const_index_err(e: &err::ErrKind) -> bool {
        matches!(
            e,
            err::ErrKind::CompileErr(c) if matches!(**c, CompileErr::InvalidIndexIsNotConst(..))
        )
    }
}
