//! 条件分岐の契約チェック
//!
//! 一つの分岐で契約を終了するなら、すべての分岐で終了しなければならない

use super::*;
use crate::compiler::ast::{Expr, ExprNode, Group2Node, StmtNode};
use crate::err::compile::CompileErrKind::ConstractUnterminableScope;

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
    /// ループ本体の中の条件分岐(`Expr::Match`)の追跡
    ///
    /// `must_vars`はループの外で定義された`must`の変数。
    /// 戻り値は、この分岐の式(`cond`の条件など)で終了した変数名
    pub(super) fn tracking_cond_expr_node(
        &mut self,
        expr: &Expr,
        must_vars: &MustVarsTy,
        fn_name: &str,
    ) -> Result<Vec<String>, err::ErrKind> {
        let ExprNode::Match {
            pattern,
            arms,
            arm_else,
        } = &expr.node
        else {
            return Ok(Vec::new());
        };
        let mut ends: Vec<String> = Vec::new();
        // 分岐の式(`cond`の条件など)で終了するなら、この文で終了する
        if let Some(pattern) = pattern {
            ends = self.ended_by(Self::as_expr(pattern), must_vars, fn_name)?;
            // 終了する契約を反映
            for e in ends.iter() {
                Self::mark_used(e, must_vars);
            }
        }
        for arm in arms.iter() {
            // アームのパターン式で終了するなら、この文で終了する
            // (アームごとに分岐前の状態の複製で調べ、`must_vars`は変えない)
            for name in self.ended_by(
                Self::as_expr(&arm.pattern),
                &Self::fork_vars(must_vars),
                fn_name,
            )? {
                if !ends.contains(&name) {
                    // 前回の条件のスコープで終了していない契約は終了できない
                    return crate::GenCompileErrNode!(
                        ConstractUnterminableScope,
                        fn_name.to_string(),
                        Rc::clone(&expr.span)
                    );
                }
            }
            self.check_loop_block(fn_name, &arm.body, &Self::fork_vars(must_vars))?;
        }
        if let Some(body) = arm_else {
            self.check_loop_block(fn_name, body, &Self::fork_vars(must_vars))?;
        }
        Ok(ends)
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
}
