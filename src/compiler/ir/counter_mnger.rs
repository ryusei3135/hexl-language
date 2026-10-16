//! 変数が使われた回数をカウントし、優先的にレジスタに配置する変数を
//! 決めるための情報を作る
//!
//! ## カウントの単位
//! 関数のスコープごとに1つの[`VarCounter`]を持ち、`FnDefInfo.var_counter`
//! に保存する。関数のIRを作り始めるとき(`ini_def_fn_info`)に作り直すので、
//! 別の関数の使用回数が混ざらない。
//!
//! ## 何を数えるか
//! - 普通の変数(ローカル変数): 名前ごと(`list`)
//! - 引数の変数: 左から何番目の引数かごと(`args`)
//!
//! どちらも「値の読み出し」「代入先」「配列/構造体メンバー/ポインタ経由の
//! アクセス」など、IRを作る過程で変数が参照されるたびに1回と数える
//! (定義自体は数えない)。IRの各所からは[`IR::count_var_use`]を呼ぶ。
//! ここで変数が引数かローカル変数かを判定するので、呼び出し側は区別しなくてよい。

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct VarCounter {
    /// ローカル変数の名前 → 使われた回数(使われていない変数は載らない)
    list: HashMap<String, usize>,
    /// 引数の番号(左から0始まり) → 使われた回数
    args: Vec<usize>,
}

impl VarCounter {
    pub fn new() -> Self {
        Self {
            list: HashMap::new(),
            args: Vec::new(),
        }
    }

    /// 引数が`arg_count`個ある関数用に、全ての引数の回数を0で初期化して作る
    /// (使われなかった引数も`arg_use_count`が0を返すようにするため)
    pub fn with_arg_count(arg_count: usize) -> Self {
        Self {
            list: HashMap::new(),
            args: vec![0; arg_count],
        }
    }

    /// ローカル変数が使われたことを記録する
    pub fn use_var(&mut self, var_name: &str) {
        *self.list.entry(var_name.to_string()).or_insert(0) += 1;
    }

    /// `index`番目(左から0始まり)の引数が使われたことを記録する
    pub fn use_arg(&mut self, index: usize) {
        if self.args.len() <= index {
            self.args.resize(index + 1, 0);
        }
        self.args[index] += 1;
    }

    /// ローカル変数が使われた回数(一度も使われていなければ0)
    pub fn var_use_count(&self, var_name: &str) -> usize {
        self.list.get(var_name).copied().unwrap_or(0)
    }

    /// `index`番目の引数が使われた回数(一度も使われていなければ0)
    pub fn arg_use_count(&self, index: usize) -> usize {
        self.args.get(index).copied().unwrap_or(0)
    }

    /// ローカル変数を、使われた回数が多い順に並べて返す
    /// (同じ回数なら名前順。レジスタに優先的に置きたい順)
    pub fn vars_by_priority(&self) -> Vec<(&str, usize)> {
        let mut vars: Vec<(&str, usize)> = self
            .list
            .iter()
            .map(|(name, count)| (name.as_str(), *count))
            .collect();
        vars.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        vars
    }

    /// 引数を、使われた回数が多い順に並べて`(引数の番号, 回数)`で返す
    /// (同じ回数なら番号が小さい順)
    pub fn args_by_priority(&self) -> Vec<(usize, usize)> {
        let mut args: Vec<(usize, usize)> = self.args.iter().copied().enumerate().collect();
        args.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        args
    }
}

impl IR {
    /// 変数`var_name`が使われたことを、現在処理中の関数のカウンターに記録する
    ///
    /// 変数が引数なら`args`に、ローカル変数なら`list`に数える。
    /// 登録されていない名前は数えない(未定義エラーは、各所の
    /// 型の取得で報告されるのでここでは扱わない)。
    pub(in crate::compiler::ir) fn count_var_use(&mut self, var_name: &str) {
        match self.var_tree.find(var_name) {
            Some(def_tree::VarType::Param(index)) => {
                let index = *index;
                self.var_counter.use_arg(index);
            }
            Some(def_tree::VarType::Local(_)) => self.var_counter.use_var(var_name),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_start_from_one_use() {
        let mut c = VarCounter::new();
        assert_eq!(c.var_use_count("a"), 0);
        c.use_var("a");
        assert_eq!(c.var_use_count("a"), 1);
        c.use_var("a");
        assert_eq!(c.var_use_count("a"), 2);
    }

    #[test]
    fn args_are_counted_by_index() {
        let mut c = VarCounter::with_arg_count(3);
        c.use_arg(1);
        c.use_arg(1);
        c.use_arg(2);
        assert_eq!(c.arg_use_count(0), 0);
        assert_eq!(c.arg_use_count(1), 2);
        assert_eq!(c.arg_use_count(2), 1);
        assert_eq!(c.args_by_priority(), vec![(1, 2), (2, 1), (0, 0)]);
    }

    #[test]
    fn vars_by_priority_orders_by_count_then_name() {
        let mut c = VarCounter::new();
        c.use_var("b");
        c.use_var("a");
        c.use_var("c");
        c.use_var("c");
        assert_eq!(c.vars_by_priority(), vec![("c", 2), ("a", 1), ("b", 1)]);
    }
}
