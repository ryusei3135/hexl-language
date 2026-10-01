//! 配列/ポインタへのアクセスと、ポインタ変数の加算を静的に検査する
//!
//! `gen_expr_ir`(`src/ir/builder.rs`)が式を処理するときに、
//! `check_mem_access`を一度呼び出す。範囲の判定そのものは
//! `tracking.rs`のチェッカー群と`var_tree`に記録された値を使う。

use super::*;

impl IR {
    /// 配列/ポインタに関わる式を処理するときに呼び出す検査
    pub(in crate::ir) 
    fn check_mem_access(&mut self, expr: &node::Expr) {
        todo!("配列/ポインタのアクセスに関する検査を追加する");
        match expr {
            node::Expr::RefArray { name, index, .. } => {
                self.check_index_is_const(name, index);
                self.arr_idx_checker(name, index);
                self.range_ptr_checker(name, index);
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
    }

    /// 添字が即値、または`const`の変数(とその四則演算)だけで
    /// 構成されているかを確認する
    ///
    /// それ以外(`mut`や属性なしの変数、関数呼び出しなど)が含まれる場合は、
    /// その場で`panic`する。
    /// TODO: 定数でない添字(実行時に決まる添字)の処理を追加する
    fn check_index_is_const(&self, name: &str, index: &node::Expr) {
        if !self.is_const_or_immediate(index) {
            panic!(
                "`{}` の添字は即値か`const`の変数である必要があります: {:?}",
                name, index,
            );
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
        if !self.is_const_index(whole) {
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
