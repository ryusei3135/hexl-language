//! 条件分岐の契約チェック
//!
//! 一つの分岐で契約を終了するなら、すべての分岐で終了しなければならない

use super::*;

impl IR {
    /// 条件分岐(`node::Expr::Match`)の追跡
    ///
    /// 分岐ごとに独立して追跡し、最後に結果を突き合わせる。
    /// 一つでも契約を終了する分岐があるなら、すべての分岐で
    /// 終了しなければならない
    pub(super) fn walk_match(expr: &node::Expr, must_vars: &mut Vec<MustVar>) {
        let node::Expr::Match {
            pattern,
            arms,
            arm_else,
        } = expr
        else {
            return;
        };
        if let Some(pattern) = pattern {
            Self::walk_expr(pattern, must_vars);
        }
        // (分岐の解析結果, `ret`/`break`で抜ける分岐か)
        let mut branches: Vec<(Vec<MustVar>, bool)> = Vec::new();
        for arm in arms.iter() {
            let state = Self::walk_branch(&must_vars[..], |vars| {
                Self::walk_expr(&arm.pattern, vars);
                Self::walk_group2_nodes(&arm.body, vars);
            });
            branches.push((state, Self::ends_with_exit(&arm.body)));
        }
        if let Some(body) = arm_else {
            let state = Self::walk_branch(&must_vars[..], |vars| {
                Self::walk_group2_nodes(body, vars);
            });
            branches.push((state, Self::ends_with_exit(body)));
        } else if pattern.is_none() {
            // `else`の無い条件分岐は、何もしない分岐があるのと同じ
            branches.push((must_vars.clone(), false));
        }
        Self::merge_branches(must_vars, &branches);
    }

    /// 分岐1つ分を、分岐前の状態のコピーに対して解析して返す
    /// (分岐の中で定義された変数は結果に含めない)
    fn walk_branch(must_vars: &[MustVar], walk: impl FnOnce(&mut Vec<MustVar>)) -> Vec<MustVar> {
        let outer_len = must_vars.len();
        let mut local = must_vars.to_vec();
        walk(&mut local);
        local.truncate(outer_len);
        local
    }

    /// 本体の最後の文が`ret`/`break`か
    fn ends_with_exit(body: &Body) -> bool {
        matches!(
            body.iter().last().map(|n| n.get_node()),
            Some(node::Group2Node::Stmt(
                node::StmtNode::Return(_) | node::StmtNode::Break
            ))
        )
    }

    /// 各分岐の解析結果を`must_vars`に反映する
    ///
    /// - すべての分岐で終了 -> 終了済み
    /// - 一部の分岐でだけ終了 -> 終了済みにしたうえで`branch_mismatch`を立てる
    /// - どの分岐でも終了していない -> 変化なし
    /// - `ret`/`break`で抜ける分岐で終了している場合は、その分岐を比較から外す
    ///   (`cond c { f(x)\n break } else {}`のように、抜けない側は後で終了できる)
    fn merge_branches(must_vars: &mut Vec<MustVar>, branches: &[(Vec<MustVar>, bool)]) {
        for (i, var) in must_vars.iter_mut().enumerate() {
            // 分岐の前に終了済みなら、分岐ごとの確認は不要
            if var.used {
                continue;
            }
            if branches.iter().any(|br| br.0[i].branch_mismatch) {
                var.branch_mismatch = true;
            }
            if branches.iter().any(|br| br.0[i].ended_on_exit) {
                var.ended_on_exit = true;
            }
            let exit_ended = branches.iter().any(|br| br.1 && br.0[i].used);
            let rest: Vec<&(Vec<MustVar>, bool)> = branches
                .iter()
                .filter(|br| !(br.1 && br.0[i].used))
                .collect();
            let used_count = rest.iter().filter(|br| br.0[i].used).count();
            if used_count > 0 {
                var.used = true;
                if used_count < rest.len() {
                    var.branch_mismatch = true;
                }
            } else if exit_ended {
                var.ended_on_exit = true;
            }
        }
    }
}
