//! 契約の追跡で、条件分岐・反復処理に共通する処理
//! (`must`の変数の追跡状態と、式を辿る処理)

use super::*;
use crate::compiler::ast::{Expr, ExprNode, Group2Node, StmtNode};

/// 関数の中で追跡している、`must`の契約を持つ変数
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct MustVar {
    pub(super) name: String,
    /// 一度でも関数の引数として渡されたら`true`
    pub(super) used: bool,
    /// 条件分岐の一部の分岐でしか契約を終了していなければ`true`
    /// (すべての分岐で終了する必要があるのでエラー)
    pub(super) branch_mismatch: bool,
    /// `ret`/`break`で関数やループを抜ける分岐の中でだけ終了していれば`true`
    /// (抜けない側の分岐が終了しなくても、分岐の不一致とは扱わない)
    pub(super) ended_on_exit: bool,
    pub(super) span: Rc<Span>,
}

impl MustVar {
    pub(super) fn new(name: String, span: &Rc<Span>) -> Self {
        Self {
            name,
            used: false,
            branch_mismatch: false,
            ended_on_exit: false,
            span: Rc::clone(&span),
        }
    }
}

impl IR {
    pub(super) fn walk_group2_nodes(
        &mut self,
        nodes: &Body,
        must_vars: &mut Vec<MustVar>,
        fn_name: &str,
    ) -> Result<(), err::ErrKind> {
        for n in nodes.iter() {
            match n.get_node() {
                Group2Node::Expr(expr) => {
                    self.walk_expr(&expr, must_vars, fn_name)?;
                }
                Group2Node::Stmt(StmtNode::Return(expr)) => {
                    self.walk_expr(&expr, must_vars, fn_name)?;
                }
                Group2Node::Stmt(StmtNode::Continue | StmtNode::Break(..)) => {}
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn walk_expr(
        &mut self,
        expr: &Expr,
        must_vars: &mut Vec<MustVar>,
        fn_name: &str,
    ) -> Result<(), err::ErrKind> {
        match &expr.node {
            // 変数の定義: `must`の型ならここから追跡を始める
            ExprNode::DefVar(var) => {
                self.walk_expr(&var.value, must_vars, fn_name)?;
                if var.ty.is_constract_must() {
                    if must_vars.iter().rev().any(|entry| entry.name == var.name) {
                        // 同名の契約変数がブロックや分岐ごとに再定義された場合、
                        // 直近のスコープで使われる変数だけを追跡対象にする。
                        let shadowed = must_vars
                            .iter_mut()
                            .rev()
                            .find(|entry| entry.name == var.name)
                            .unwrap();
                        shadowed.used = false;
                    } else {
                        must_vars.push(MustVar::new(var.name.clone(), &expr.span));
                    }
                }
                Ok(())
            }
            // 関数の呼び出し: 引数に渡された`must`の変数を消費済みにする
            ExprNode::CallFunc(call_info) => {
                call_info.args.iter().try_for_each(|arg| {
                    if let Some(name) = Self::arg_var_name(arg) {
                        Self::mark_used(name, must_vars);
                    }
                    self.walk_expr(arg, must_vars, fn_name)
                })?;
                Ok(())
            }
            ExprNode::Scope { target, .. }
            | ExprNode::Member { target, .. }
            | ExprNode::PtrMember { target, .. } => self.walk_expr(target, must_vars, fn_name),
            ExprNode::Assign(assign) => {
                self.walk_expr(&assign.dst, must_vars, fn_name)?;
                self.walk_expr(&assign.value, must_vars, fn_name)
            }
            ExprNode::GetAddress(target) | ExprNode::ConnectAddr(target) => {
                self.walk_expr(target, must_vars, fn_name)
            }
            ExprNode::VariantBind(bind) => self.walk_expr(&bind.value, must_vars, fn_name),
            ExprNode::Add(pair)
            | ExprNode::Sub(pair)
            | ExprNode::Mul(pair)
            | ExprNode::Div(pair)
            | ExprNode::Surplus(pair)
            | ExprNode::LessThen(pair)
            | ExprNode::GreaterThen(pair)
            | ExprNode::Equal(pair)
            | ExprNode::NotEq(pair) => {
                self.walk_expr(&pair.0, must_vars, fn_name)?;
                self.walk_expr(&pair.1, must_vars, fn_name)
            }
            // 条件分岐は`branch.rs`、反復処理は`loop_check.rs`で扱う
            ExprNode::Match { .. } => self.walk_match(expr, must_vars, fn_name),
            ExprNode::Loop { .. } => self.walk_loop(expr, must_vars, fn_name),
            ExprNode::InitStruct { fields, .. } => {
                let f = |val: &Box<Expr>| -> Result<(), err::ErrKind> {
                    self.walk_expr(val, must_vars, fn_name)
                };
                fields.values().try_for_each(f)?;
                Ok(())
            }
            ExprNode::Array(values) => {
                let f = |val: &Expr| -> Result<(), err::ErrKind> {
                    self.walk_expr(val, must_vars, fn_name)
                };
                values.iter().try_for_each(f)?;
                Ok(())
            }
            ExprNode::RefArray { dst, index, .. } => {
                self.walk_expr(dst, must_vars, fn_name)?;
                self.walk_expr(index, must_vars, fn_name)
            }
            _ => Ok(()),
        }
    }

    /// 引数の式が指している変数の名前を返す
    /// (`&var`や`*var`のように包まれていても中身を辿る)
    pub(super) fn arg_var_name(arg: &Expr) -> Option<&String> {
        match &arg.node {
            ExprNode::Var(name) => Some(name),
            ExprNode::GetAddress(target) | ExprNode::ConnectAddr(target) => {
                Self::arg_var_name(&target)
            }
            _ => None,
        }
    }

    pub(super) fn mark_used(name: &str, must_vars: &mut Vec<MustVar>) {
        if let Some(ref mut var) = must_vars.iter_mut().rev().find(|var| &var.name == name) {
            var.used = true;
        }
    }
}
