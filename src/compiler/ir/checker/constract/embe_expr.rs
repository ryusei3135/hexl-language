//! 式の中に組み込まれている契約された変数を追跡する

use super::*;
use crate::compiler::ast::{Expr, ExprNode};
use crate::compiler::err::CompileErrKind::ConstractTernaryMismatch;

impl IR {
    /// 三項演算子(`cond ? left : right`)の両辺の契約が揃っているか調べる
    ///
    /// - 両辺とも契約(`must`)の値 -> `true`
    /// - 両辺とも契約を持たない値(即値・算術など) -> `true`
    /// - 片方だけが契約の値 -> `false`
    ///
    /// どっちかが即値などならどっちも明示的に契約した型は使えない
    /// どっちかの変数が契約ならどっちも契約である必要がある
    pub(super) fn check_tarnary_contract(
        left: &Box<Expr>,
        right: &Box<Expr>,
        span: &Rc<Span>,
    ) -> Result<(), err::ErrKind> {
        match (
            Self::is_contract_value(left),
            Self::is_contract_value(right),
        ) {
            (Some(l), Some(r)) if l == r => Ok(()),
            // 片方だけが契約の値、または入れ子の三項演算子の中ですでに揃っていない
            _ => crate::GenCompileErr2!(ConstractTernaryMismatch, Rc::clone(span)),
        }
    }

    /// 式の値が契約(`must`)を持つか
    ///
    /// 内側に揃っていない三項演算子があり、判定できないときは`None`。
    /// 型を確定できない式(`Unresolved`)は、`check_constract_arg`と同じく
    /// `must`の値とは認めず、契約を持たないものとして扱う。
    fn is_contract_value(expr: &Expr) -> Option<bool> {
        match &expr.node {
            ExprNode::Ternary(n) => {
                // `n.0`は条件。契約の有無を見るのは両辺(`n.1`/`n.2`)
                let l = Self::is_contract_value(&n.1)?;
                let r = Self::is_contract_value(&n.2)?;
                (l == r).then_some(l)
            }
            _ => Some(true),
        }
    }
}
