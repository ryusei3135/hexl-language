//! 配列やポインタが契約した範囲にいるかを
//! 探索

use super::*;

impl IR {
    /// 範囲付きポインタや配列の長さを超えて数を指定しているかを
    /// 調べるためにインタプリタを実行
    ///
    /// 変数の参照(`node::Expr::Var`)は、`record_var_value`で
    /// `var_tree`に記録されている現在値に置き換えて評価する。
    /// 呼び出し前に`is_const_index`で評価可能かどうかを確認しておくこと
    /// (記録の無い変数を評価しようとした場合はここで`panic`する)
    fn eval_expr(&self, n: &node::Expr) -> usize {
        match n {
            node::Expr::Number(val) => val.parse::<usize>().unwrap(),
            node::Expr::Var(name) => self.var_tree.get_value(name).unwrap_or_else(|| {
                panic!(
                    "変数 `{}` の値が記録されておらず、静的に評価できません",
                    name
                )
            }),
            node::Expr::Add((l, r)) => self.eval_expr(l) + self.eval_expr(r),
            node::Expr::Sub((l, r)) => self.eval_expr(l) - self.eval_expr(r),
            node::Expr::Mul((l, r)) => self.eval_expr(l) * self.eval_expr(r),
            node::Expr::Div((l, r)) => self.eval_expr(l) / self.eval_expr(r),
            _ => panic!(),
        }
    }

    /// 添字/代入値の式が、コンパイル時に値の分かる定数式(`eval_expr`で
    /// 評価できる形)かどうかを判定する
    ///
    /// `node::Expr::Var`は、`var_tree`にその変数の現在値が
    /// 記録されている(`record_var_value`で記録済み)場合にのみ定数式として
    /// 扱う。記録が無い変数(実行時にしか値の分からない式で更新された
    /// 変数や、一度も記録されていない変数)を含む場合は、実行時にしか
    /// 値が分からないため、ここでの静的な範囲チェックの対象外として扱う
    fn is_const_index(&self, n: &node::Expr) -> bool {
        match n {
            node::Expr::Number(_) => true,
            node::Expr::Var(name) => self.var_tree.get_value(name).is_some(),
            node::Expr::Add((l, r))
            | node::Expr::Sub((l, r))
            | node::Expr::Mul((l, r))
            | node::Expr::Div((l, r)) => self.is_const_index(l) && self.is_const_index(r),
            _ => false,
        }
    }

    /// 配列のidx指定が範囲を超えていないか
    ///
    /// `name`は`name[index]`でアクセスされている配列変数の名前、
    /// `index`はその添字部分のAST。
    /// 添字が定数式でない場合や`name`が配列型でない場合は判定できない
    /// ため`true`を返す(配列型でない場合は`range_ptr_checker`に任せる)
    pub(in crate::ir) fn arr_idx_checker(&mut self, name: &str, index: &node::Expr) -> bool {
        if self.is_const_index(index) == false {
            return true;
        }

        let ty = self.var_tree.get_ty_node(name).unwrap();
        let len = match types::Size::new(&ty) {
            Some(types::Size::Array { len, .. }) => len,
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
    pub(in crate::ir) fn range_ptr_checker(&mut self, name: &str, index: &node::Expr) -> bool {
        if self.is_const_index(index) == false {
            return true;
        }

        let ty = self.var_tree.get_ty_node(name).unwrap();
        let range = match types::Size::new(&ty) {
            Some(types::Size::Pointer {
                range: Some(range), ..
            }) => range,
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

    /// 範囲付きポインタ変数への再代入が範囲を超えていないか
    ///
    /// `name`は再代入されるポインタ変数の名前、`value`はその新しい値の式
    /// (`a += 10`のような複合代入も、右辺が`a + 10`のような式へ展開された
    /// 状態でここへ渡ってくる想定)。`range_ptr_checker`が添字アクセス
    /// (`a[idx]`)を範囲チェックするのに対し、こちらは変数自体に書き込まれる
    /// 値を同じ`Size::Pointer.range`と照らし合わせてチェックする。
    /// 値が定数式として評価できない場合や、`name`が範囲指定付きの
    /// ポインタ型でない場合は判定できないため`true`を返す
    pub(in crate::ir) fn range_ptr_reassign_checker(
        &mut self,
        name: &str,
        value: &node::Expr,
    ) -> bool {
        if self.is_const_index(value) == false {
            return true;
        }

        let ty = self.var_tree.get_ty_node(name).unwrap();
        let range = match types::Size::new(&ty) {
            Some(types::Size::Pointer {
                range: Some(range), ..
            }) => range,
            // 範囲指定のないポインタ、またはポインタ型でなければ対象外
            _ => return true,
        };

        let new_value = self.eval_expr(value);
        if new_value < range.0 || new_value >= range.1 {
            panic!(
                "範囲付きポインタ `{}` への再代入が範囲外です: 値 {} (有効範囲 {}..{})",
                name, new_value, range.0, range.1,
            );
        }
        true
    }

    /// `var_tree`に登録済みの型から、範囲付きポインタ/配列の長さを取得する
    ///
    /// `name`の初期値が配列/文字列リテラルではない(`get_ast_len`で
    /// 長さを求められない)場合に、代わりに宣言時の型情報(範囲指定や
    /// 配列の長さ)から長さを求めるために使う
    /// (`src/ir/builder/expr_node.rs`の`def_var_node_with_register_ty`)。
    /// `name`が範囲指定付きのポインタ型でも配列型でもない場合はpanicする
    pub(in crate::ir) fn range_len_from_var_tree(&self, name: &str) -> usize {
        let ty = self.var_tree.get_ty_node(name).unwrap();
        match types::Size::new(&ty) {
            Some(types::Size::Pointer {
                range: Some(range), ..
            }) => range.1 - range.0,
            Some(types::Size::Array { len, .. }) => len,
            _ => panic!(
                "変数 `{}` の長さを決定できません: 配列/文字列リテラルでの初期化か、範囲の指定が必要です",
                name,
            ),
        }
    }

    /// 変数の値を更新するときに、その値を記録する
    ///
    /// `range_ptr_reassign_checker`や`arr_idx_checker`/`range_ptr_checker`が
    /// 変数を含む式を静的に評価できるようにするため、変数の定義
    /// (`def_var_node`)や再代入(`assign_expr_node`)で新しい値が
    /// 書き込まれるたびに呼び出す。
    /// `value`が定数式として評価できる場合はその結果を記録し、
    /// 評価できない(実行時にしか値が分からない)場合は、古い記録が
    /// 残って以降の静的チェックに誤って使われないよう記録を消す
    pub(in crate::ir) fn record_var_value(&mut self, name: &str, value: &node::Expr) {
        if self.is_const_index(value) {
            let v = self.eval_expr(value);
            self.var_tree.record_value(name, v);
        } else {
            self.var_tree.forget_value(name);
        }
    }
}
