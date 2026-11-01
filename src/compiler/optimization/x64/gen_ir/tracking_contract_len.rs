//! `GetContractMemLen`の長さを求める(追跡する)API
//!
//! `constant_fold.rs`の`tracking_expr_node`から呼ばれる。
//!
//! - 文字列・配列リテラルは、その長さ・要素数
//! - 変数は、IR(`VarTree`)の情報から長さを引く。引数や別の変数から
//!   受け取った変数は、元の変数まで`VarOrigin::Alias`をたどる
//! - 長さが分からない場合は`0`
//!
//! `VarTree`の中身に依存するのは`var_origin_from_tree`だけ。

use super::*;
use crate::compiler::ast;
use crate::compiler::ir::{def_tree, types::Size};

/// 変数名から、その変数(配列・文字列)の長さを返す関数。
/// 長さが分からない変数は`None`を返す。
pub type VarLenFn<'a> = &'a dyn Fn(&str) -> Option<u64>;

/// `GetContractMemLen(var)`の長さを返す。
///
/// - 文字列リテラル → 文字列の長さ(バイト数)
/// - 配列リテラル → 要素数
/// - 変数 → `var_len`(変数の情報)から引いた長さ
/// - それ以外・長さが分からない場合 → `0`
pub fn contract_mem_len(var: &ast::Expr, var_len: VarLenFn) -> Option<u64> {
    use ast::ExprNode;
    match &var.node {
        ExprNode::Str(v) => Some(v.len() as u64),
        ExprNode::Array(v) => Some(v.len() as u64),
        ExprNode::Var(name) => var_len(name),
        _ => return None,
    }
}

/// 変数の長さの「出どころ」。IR(`VarTree`)の情報から作る。
#[derive(Clone, Debug, PartialEq)]
pub enum VarOrigin {
    /// 長さが分かっている(配列型の変数・引数の型に書かれた長さなど)
    Len(u64),
    /// 別の変数の値を受け取っている(引数・`b = a`のような代入など)。
    /// その変数の長さをたどる
    Alias(String),
}

/// 変数`name`の長さを、`origin`(IRの情報)をたどって求める。
///
/// 引数や別の変数から受け取った変数(`Alias`)は、元の変数までたどる。
/// 長さが分からない・たどれない・循環している場合は`None`。
pub fn resolve_var_len(name: &str, origin: &dyn Fn(&str) -> Option<VarOrigin>) -> Option<u64> {
    let mut visited: Vec<String> = Vec::new();
    let mut curr = name.to_string();
    loop {
        // `a -> b -> a`のような循環で止まらなくならないようにする
        if visited.contains(&curr) {
            return None;
        }
        match origin(&curr)? {
            VarOrigin::Len(len) => return Some(len),
            VarOrigin::Alias(next) => {
                visited.push(curr);
                curr = next;
            }
        }
    }
}

/// 変数の情報(`VarTree`)から、変数の長さの出どころを引く。
///
/// TODO: `VarTree`/`Size::Array`の実際のAPIに合わせてここだけ直す。
/// (ここ以外は`VarTree`の中身に依存しない)
/// - 配列型(`Size::Array { len, .. }`)なら、型に書かれた長さ
/// - そうでない変数は、値の受け取り元の変数(`src_var`)があればそれをたどる
fn var_origin_from_tree(name: &str, var_tree: &def_tree::VarTree) -> Option<VarOrigin> {
    let meta = var_tree.get_var(name).ok()?;
    if let Ok(ref ty) = meta.size.try_into() {
        if let Size::Array { len, .. } = ty {
            return u64::try_from(*len).ok().map(VarOrigin::Len);
        }
    }
    None
}

/// 変数の情報(`VarTree`)から、変数名で長さを引く(引数などはたどる)。
pub fn var_len_from_tree(name: &str, var_tree: &def_tree::VarTree) -> Option<u64> {
    resolve_var_len(name, &|n| var_origin_from_tree(n, var_tree))
}

#[cfg(test)]
mod tests {
    use super::super::constant_fold::{ValKind, constant_folding};
    use super::*;
    use ast::ExprNode;
    use std::rc::Rc;

    fn sp() -> Rc<crate::compiler::err::Span> {
        Rc::new(crate::compiler::err::Span::unknown())
    }

    fn num(v: &str) -> ast::Expr {
        ExprNode::Number(v.to_string()).with_span(sp())
    }

    /// 変数の長さが何も分からない
    fn no_var(_: &str) -> Option<u64> {
        None
    }

    #[test]
    fn contract_mem_len_of_literals_and_vars() {
        let arr = ExprNode::Array(vec![num("1"), num("2"), num("3")]).with_span(sp());
        let len = |e: ast::Expr| ExprNode::GetContractMemLen(Box::new(e)).with_span(sp());
        assert_eq!(constant_folding(&len(arr), &no_var), Some(ValKind::Num(3)));

        let s = ExprNode::Str("abcd".to_string()).with_span(sp());
        assert_eq!(constant_folding(&len(s), &no_var), Some(ValKind::Num(4)));

        // 変数: 長さが分かれば その長さ、分からなければ0
        let var = || ExprNode::Var("a".to_string()).with_span(sp());
        let known = |name: &str| (name == "a").then_some(5_u64);
        assert_eq!(constant_folding(&len(var()), &known), Some(ValKind::Num(5)));
        assert_eq!(
            constant_folding(&len(var()), &no_var),
            Some(ValKind::Num(0))
        );
    }

    #[test]
    fn resolve_var_len_follows_aliases() {
        use std::collections::HashMap;
        let mut m: HashMap<&str, VarOrigin> = HashMap::new();
        m.insert("arr", VarOrigin::Len(4));
        m.insert("param", VarOrigin::Alias("arr".to_string()));
        m.insert("copy", VarOrigin::Alias("param".to_string()));
        m.insert("loop_a", VarOrigin::Alias("loop_b".to_string()));
        m.insert("loop_b", VarOrigin::Alias("loop_a".to_string()));
        m.insert("dangling", VarOrigin::Alias("nothing".to_string()));
        let origin = |n: &str| m.get(n).cloned();

        assert_eq!(resolve_var_len("arr", &origin), Some(4));
        assert_eq!(resolve_var_len("copy", &origin), Some(4));
        // 循環・たどれない・存在しない変数は`None`(`GetContractMemLen`では0になる)
        assert_eq!(resolve_var_len("loop_a", &origin), None);
        assert_eq!(resolve_var_len("dangling", &origin), None);
        assert_eq!(resolve_var_len("none", &origin), None);

        // 追跡した結果を`GetContractMemLen`で使う
        let var_len = |n: &str| resolve_var_len(n, &origin);
        let len = |name: &str| {
            ExprNode::GetContractMemLen(Box::new(ExprNode::Var(name.to_string()).with_span(sp())))
                .with_span(sp())
        };
        assert_eq!(
            constant_folding(&len("copy"), &var_len),
            Some(ValKind::Num(4))
        );
        assert_eq!(
            constant_folding(&len("loop_a"), &var_len),
            Some(ValKind::Num(0))
        );
    }
}
