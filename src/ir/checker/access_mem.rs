//! 配列/ポインタへのアクセスと、ポインタ変数の加算を静的に検査する
//!
//! `gen_expr_ir`(`src/ir/builder.rs`)が式を処理するときに、
//! `check_mem_access`を一度呼び出す。範囲の判定そのものは
//! `tracking.rs`のチェッカー群と`var_tree`に記録された値を使う。

use super::*;


/// `check_mem_access`が返す、配列アクセスに挿入すべき実行時ガード
///
/// 添字が`const`/即値のときは静的検査だけで済むため`None`が返り、
/// IRはそのまま生成される。添字が可変のときだけこれが返る。
///
/// IR側では次の形になるように条件分岐のノードを差し込む:
/// ```text
/// if lo <= index && index < hi { arr[index] } else { arr[else_idx] }
/// ```
#[derive(Clone, Debug)]
pub(in crate::ir) 
struct ArrIdxGuard {
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
    /// - 添字が`const`/即値: 静的に範囲検査してそのまま(`None`)
    /// - 添字が可変: 範囲内のときだけ通る条件分岐を差し込むための
    ///   `ArrIdxGuard`を返す。条件がfalseなら`else_idx`が使われる
    pub(in crate::ir)
    fn check_mem_access(&mut self, expr: &node::Expr) -> Option<ArrIdxGuard> {
        match expr {
            node::Expr::RefArray { name, index, else_idx, .. } => {
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
        None
    }

    /// `name[index else else_idx]`の検査
    ///
    /// ## Args
    /// - else_idx: `[arr idx else 0]`配列のidxが範囲外のときのidx
    fn check_ref_array(
        &mut self,
        name: &str,
        index: &node::Expr,
        else_idx: &Option<Box<node::Expr>>,
    ) -> Option<ArrIdxGuard> {
        // 配列の長さ(範囲付きポインタなら範囲)を測る。どちらでもなければ対象外
        let (lo, hi) = self.access_bounds(name)?;

        // 添字が`const`/即値: 静的に検査するだけで、IRはそのまま
        if self.is_const_or_immediate(index) {
            self.arr_idx_checker(name, index);
            self.range_ptr_checker(name, index);
            return None;
        }

        // 添字が可変: `else_idx`が必須で、かつ`const`/即値でなければならない
        let Some(else_expr) = else_idx.as_ref().map(|v| &**v) else {
            panic!(
                "`{}` の添字が可変なので、範囲外のときの添字(`else`)が必要です: {:?}",
                name, index,
            );
        };
        if self.is_const_or_immediate(else_expr) == false {
            panic!(
                "`{}` の`else`の添字は即値か`const`の変数である必要があります: {:?}",
                name, else_expr,
            );
        }

        // 有効な範囲が空だと、どの添字でもアクセスできない
        // (IR側で`hi - 1`を使うので、ここで弾いておく)
        if hi <= lo {
            panic!(
                "`{}` の有効範囲が空なので、可変な添字ではアクセスできません: {}..{}",
                name, lo, hi,
            );
        }

        // `else_idx`自体が範囲外だと、フォールバックが範囲外アクセスになる
        if self.is_const_index(else_expr) {
            let v = self.eval_expr(else_expr);
            if v < lo || v >= hi {
                panic!(
                    "配列 `{}` の`else`の添字が範囲外です: 添字 {} (有効範囲 {}..{})",
                    name, v, lo, hi,
                );
            }
        }

        Some(ArrIdxGuard {
            name: name.to_string(),
            lo,
            hi,
            index: index.clone(),
            else_idx: else_expr.clone(),
        })
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

    /// 式が即値か`const`の変数、またはそれらの四則演算かどうか
    fn is_const_or_immediate(&self, expr: &node::Expr) -> bool {
        match expr {
            node::Expr::Number(_) => true,
            node::Expr::Var(name) => matches!(
                self.var_tree.is_mut(&name.to_string()),
                crate::parse::VarMutAttr::Const
            ),
            node::Expr::Add((l, r))
            | node::Expr::Sub((l, r))
            | node::Expr::Mul((l, r))
            | node::Expr::Div((l, r)) => {
                self.is_const_or_immediate(l) && self.is_const_or_immediate(r)
            }
            _ => false,
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