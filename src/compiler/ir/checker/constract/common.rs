//! 契約の追跡で、条件分岐・反復処理に共通する処理
//! (`must`の変数の追跡状態と、式を辿る処理)

use super::*;

/// 関数の中で追跡している、`must`の契約を持つ変数
#[derive(Clone, Debug)]
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
}

impl MustVar {
    pub(super) fn new(name: String) -> Self {
        Self {
            name,
            used: false,
            branch_mismatch: false,
            ended_on_exit: false,
        }
    }
}

impl IR {
    pub(super) fn walk_group2_nodes(nodes: &Body, must_vars: &mut Vec<MustVar>) {
        for n in nodes.iter() {
            match n.get_node() {
                node::Group2Node::Expr(expr) => {
                    Self::walk_expr(&expr, must_vars);
                }
                node::Group2Node::Stmt(node::StmtNode::Return(expr)) => {
                    Self::walk_expr(&expr, must_vars);
                }
                node::Group2Node::Stmt(node::StmtNode::Continue | node::StmtNode::Break) => {}
                _ => {}
            }
        }
    }

    pub(super) fn walk_expr(expr: &node::Expr, must_vars: &mut Vec<MustVar>) {
        match &expr.node {
            // 変数の定義: `must`の型ならここから追跡を始める
            node::ExprNode::DefVar(var) => {
                Self::walk_expr(&var.value, must_vars);
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
                        must_vars.push(MustVar::new(var.name.clone()));
                    }
                }
            }
            // 関数の呼び出し: 引数に渡された`must`の変数を消費済みにする
            node::ExprNode::CallFunc(call_info) => {
                call_info.args.iter().for_each(|arg| {
                    if let Some(name) = Self::arg_var_name(arg) {
                        Self::mark_used(name, must_vars);
                    }
                    Self::walk_expr(arg, must_vars);
                });
            }
            node::ExprNode::Scope { target, .. }
            | node::ExprNode::Member { target, .. }
            | node::ExprNode::PtrMember { target, .. } => {
                Self::walk_expr(target, must_vars);
            }
            node::ExprNode::Assign(assign) => {
                Self::walk_expr(&assign.dst, must_vars);
                Self::walk_expr(&assign.value, must_vars);
            }
            node::ExprNode::GetAddress(target) | node::ExprNode::ConnectAddr(target) => {
                Self::walk_expr(target, must_vars);
            }
            node::ExprNode::VariantBind(bind) => {
                Self::walk_expr(&bind.value, must_vars);
            }
            node::ExprNode::Add(pair)
            | node::ExprNode::Sub(pair)
            | node::ExprNode::Mul(pair)
            | node::ExprNode::Div(pair)
            | node::ExprNode::Surplus(pair)
            | node::ExprNode::LessThen(pair)
            | node::ExprNode::GreaterThen(pair)
            | node::ExprNode::Equal(pair)
            | node::ExprNode::NotEq(pair) => {
                Self::walk_expr(&pair.0, must_vars);
                Self::walk_expr(&pair.1, must_vars);
            }
            // 条件分岐は`branch.rs`、反復処理は`loop_check.rs`で扱う
            node::ExprNode::Match { .. } => Self::walk_match(expr, must_vars),
            node::ExprNode::Loop { .. } => Self::walk_loop(expr, must_vars),
            node::ExprNode::InitStruct { fields, .. } => {
                fields
                    .values()
                    .for_each(|val| Self::walk_expr(val, must_vars));
            }
            node::ExprNode::Array(values) => {
                values
                    .iter()
                    .for_each(|val| Self::walk_expr(val, must_vars));
            }
            node::ExprNode::RefArray { dst, index, .. } => {
                Self::walk_expr(dst, must_vars);
                Self::walk_expr(index, must_vars);
            }
            _ => {}
        }
    }

    /// 引数の式が指している変数の名前を返す
    /// (`&var`や`*var`のように包まれていても中身を辿る)
    pub(super) fn arg_var_name(arg: &node::Expr) -> Option<&String> {
        match &arg.node {
            node::ExprNode::Var(name) => Some(name),
            node::ExprNode::GetAddress(target) | node::ExprNode::ConnectAddr(target) => {
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
