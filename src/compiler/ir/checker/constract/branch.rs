//! 条件分岐の契約チェック
//!
//! 一つの分岐で契約を終了するなら、すべての分岐で終了しなければならない

use super::*;
use crate::compiler::ast::{Expr, ExprNode, Group2Node, StmtNode};

/// 本体が必ず抜ける先(`ret`は関数、`break`は反復処理を抜ける)
///
/// 分岐の後ろで契約を終了できるかは脱出先で変わり得るので、両者を区別して持つ。
/// 入れ子の分岐は、内側の分岐の脱出先の和集合になる
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Exits {
    pub(super) ret: bool,
    pub(super) brk: bool,
}

impl Exits {
    const RETURN: Self = Self {
        ret: true,
        brk: false,
    };
    const BREAK: Self = Self {
        ret: false,
        brk: true,
    };

    fn union(self, other: Self) -> Self {
        Self {
            ret: self.ret || other.ret,
            brk: self.brk || other.brk,
        }
    }
}

impl IR {
    /// 条件分岐(`Expr::Match`)の追跡
    ///
    /// 分岐ごとに独立して追跡し、最後に結果を突き合わせる。
    /// 一つでも契約を終了する分岐があるなら、すべての分岐で
    /// 終了しなければならない
    pub(super) fn walk_match(
        &mut self,
        expr: &Expr,
        must_vars: &mut Vec<MustVar>,
        fn_name: &str,
    ) -> Result<(), err::ErrKind> {
        let ExprNode::Match {
            pattern,
            arms,
            arm_else,
        } = &expr.node
        else {
            return Ok(());
        };
        if let Some(pattern) = pattern {
            self.walk_expr(pattern, must_vars, fn_name)?;
        }
        // (分岐の解析結果, 全経路が`ret`/`break`で抜ける分岐なら、その脱出先)
        let mut branches: Vec<(Vec<MustVar>, Option<Exits>)> = Vec::new();
        for arm in arms.iter() {
            let state = Self::walk_branch(&must_vars[..], |vars| {
                self.walk_expr(&arm.pattern, vars, fn_name)?;
                self.walk_group2_nodes(&arm.body, vars, fn_name)
            })?;
            branches.push((state, Self::body_exits(&arm.body)));
        }
        if let Some(body) = arm_else {
            let state = Self::walk_branch(&must_vars[..], |vars| {
                self.walk_group2_nodes(body, vars, fn_name)
            })?;
            branches.push((state, Self::body_exits(body)));
        } else if pattern.is_none() {
            // `else`の無い条件分岐は、何もしない分岐があるのと同じ
            branches.push((must_vars.clone(), None));
        }
        Self::merge_branches(must_vars, &branches);
        Ok(())
    }

    /// 分岐1つ分を、分岐前の状態のコピーに対して解析して返す
    /// (分岐の中で定義された変数は結果に含めない)
    fn walk_branch(
        must_vars: &[MustVar],
        walk: impl FnOnce(&mut Vec<MustVar>) -> Result<(), err::ErrKind>,
    ) -> Result<Vec<MustVar>, err::ErrKind> {
        let outer_len = must_vars.len();
        let mut local = must_vars.to_vec();
        walk(&mut local)?;
        local.truncate(outer_len);
        Ok(local)
    }

    /// 本体を実行すると、必ず`ret`/`break`で抜けるか(全経路の解析)
    ///
    /// 抜ける文より後ろは実行されないので、最初に「必ず抜ける」文を見つけた時点で決まる。
    /// 入れ子の条件分岐は、`else`まで含めたすべての分岐が抜けるなら抜けるとみなす。
    /// 反復処理(`loop`)の中の`break`はその反復処理を抜けるだけなので、
    /// `loop`の式は抜ける文として数えない
    pub(super) fn body_exits(body: &Body) -> Option<Exits> {
        for n in body.iter() {
            match n.get_node() {
                Group2Node::Stmt(StmtNode::Return(_)) => return Some(Exits::RETURN),
                Group2Node::Stmt(StmtNode::Break(_)) => return Some(Exits::BREAK),
                Group2Node::Expr(expr) => {
                    if let Some(exits) = Self::expr_exits(&expr) {
                        return Some(exits);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// 式を評価すると、必ず`ret`/`break`で抜けるか
    ///
    /// `else`の無い条件分岐は何も実行しない経路があるので抜けない。
    /// `pattern`のある分岐式は、すべての値を網羅しているかをここでは判定できないので、
    /// `else`が無ければ抜けないものとして扱う(抜けないとみなす側は、分岐どうしの
    /// 比較から外れないので、検査が甘くなることはない)
    pub(super) fn expr_exits(expr: &Expr) -> Option<Exits> {
        let ExprNode::Match { arms, arm_else, .. } = &expr.node else {
            return None;
        };
        let mut exits = Exits::default();
        for arm in arms.iter() {
            exits = exits.union(Self::body_exits(&arm.body)?);
        }
        Some(exits.union(Self::body_exits(arm_else.as_ref()?)?))
    }

    /// 各分岐の解析結果を`must_vars`に反映する
    ///
    /// - すべての分岐で終了 -> 終了済み
    /// - 一部の分岐でだけ終了 -> 終了済みにしたうえで`branch_mismatch`を立てる
    /// - どの分岐でも終了していない -> 変化なし
    /// - `ret`/`break`で抜ける分岐で終了している場合は、その分岐を比較から外す
    ///   (`cond c { f(x)\n break } else {}`のように、抜けない側は後で終了できる)
    fn merge_branches(must_vars: &mut Vec<MustVar>, branches: &[(Vec<MustVar>, Option<Exits>)]) {
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
            let exit_ended = branches.iter().any(|br| br.1.is_some() && br.0[i].used);
            let rest: Vec<&(Vec<MustVar>, Option<Exits>)> = branches
                .iter()
                .filter(|br| !(br.1.is_some() && br.0[i].used))
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
