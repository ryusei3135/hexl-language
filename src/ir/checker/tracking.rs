
//! 配列やポインタが契約した範囲にいるかを
//! 探索

use super::*;


impl IR {
    /// 範囲付きポインタや配列の長さを超えて数を指定しているかを
    /// 調べるためにインタプリタを実行
    fn eval_expr(&self, n: &node::Expr) -> usize {
        match n {
            node::Expr::Number(val) => {
                val.parse::<usize>().unwrap()
            }
            node::Expr::Add((l, r)) => {
                self.eval_expr(l) + self.eval_expr(r)
            }
            node::Expr::Sub((l, r)) => {
                self.eval_expr(l) - self.eval_expr(r)
            }
            node::Expr::Mul((l, r)) => {
                self.eval_expr(l) * self.eval_expr(r)
            }
            node::Expr::Div((l, r)) => {
                self.eval_expr(l) / self.eval_expr(r)
            }
            _ => panic!(),
        }
    }

    /// 添字の式が、コンパイル時に値の分かる定数式(`eval_expr`で
    /// 評価できる形)かどうかを判定する
    ///
    /// 変数などを含む添字は実行時にしか値が分からないため、
    /// ここでの静的な範囲チェックの対象外として扱う
    fn is_const_index(
        n: &node::Expr
    ) -> bool {
        match n {
            node::Expr::Number(_) => true,
            node::Expr::Add((l, r))
            | node::Expr::Sub((l, r))
            | node::Expr::Mul((l, r))
            | node::Expr::Div((l, r)) => {
                Self::is_const_index(l) && Self::is_const_index(r)
            }
            _ => false,
        }
    }

    /// 配列のidx指定が範囲を超えていないか
    ///
    /// `name`は`name[index]`でアクセスされている配列変数の名前、
    /// `index`はその添字部分のAST。
    /// 添字が定数式でない場合や`name`が配列型でない場合は判定できない
    /// ため`true`を返す(配列型でない場合は`range_ptr_checker`に任せる)
    pub(in crate::ir) fn arr_idx_checker(
        &mut self,
        name: &String,
        index: &node::Expr,
    ) -> bool {
        if Self::is_const_index(index) == false{
            return true;
        }

        let ty = self
            .var_tree
            .get_ty_node(name)
            .unwrap();
        let len = match types::Size::new(&ty) {
            Ok(types::Size::Array { len, .. }) => len,
            // 配列型でなければこのチェックの対象外
            _ => return true,
        };

        let idx_value = self.eval_expr(index);
        if idx_value >= len {
            panic!(
                "配列 `{}` の添字が範囲外です: 添字 {} (配列の長さ {})",
                name, idx_value, len,
            );
        }
        true
    }

    /// 範囲付きポインタが範囲を超えていないか
    ///
    /// `name`は`name[index]`でアクセスされているポインタ変数の名前、
    /// `index`はその添字部分のAST。
    /// 添字が定数式でない場合や、`name`が範囲指定付きのポインタ型
    /// でない場合は判定できないため`true`を返す
    pub(in crate::ir) fn range_ptr_checker(
        &mut self,
        name: &String,
        index: &node::Expr,
    ) -> bool {
        if Self::is_const_index(index) == false {
            return true;
        }

        let ty = self
            .var_tree
            .get_ty_node(name)
            .unwrap();
        let range = match types::Size::new(&ty) {
            Ok(types::Size::Pointer { range: Some(range), .. }) => range,
            // 範囲指定のないポインタ、またはポインタ型でなければ対象外
            _ => return true,
        };

        let idx_value = self.eval_expr(index);
        if idx_value < range.0 || idx_value >= range.1 {
            panic!(
                "範囲付きポインタ `{}` の添字が範囲外です: 添字 {} (有効範囲 {}..{})",
                name, idx_value, range.0, range.1,
            );
        }
        true
    }
}