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
    /// `false`なら契約を持たない変数が、同名の契約変数を隠している印
    /// (`x: int = 1`が外側の`must`の`x`をシャドーイングした場合など)。
    /// 名前の検索でこの要素に当たった変数は契約の対象ではないので、
    /// 外側の契約変数の状態を変えない。検査やエラーの対象にもならない
    pub(super) tracked: bool,
    pub(super) span: Rc<Span>,
}

impl MustVar {
    pub(super) fn new(name: String, span: &Rc<Span>) -> Self {
        Self {
            name,
            used: false,
            branch_mismatch: false,
            ended_on_exit: false,
            tracked: true,
            span: Rc::clone(&span),
        }
    }

    /// 契約を持たない変数が、同名の契約変数を隠していることを表す要素
    pub(super) fn mask(name: String, span: &Rc<Span>) -> Self {
        Self {
            tracked: false,
            ..Self::new(name, span)
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
                    // 同名の契約変数が外側にあっても、既存の要素は書き換えず
                    // 新しい要素を積む。名前の検索は後ろから行うので内側が優先され、
                    // スコープを抜けるときの`truncate`で内側の要素だけが消える
                    must_vars.push(MustVar::new(var.name.clone(), &expr.span));
                } else if must_vars.iter().any(|entry| entry.name == var.name) {
                    // 契約のない変数が外側の契約変数を隠す場合
                    must_vars.push(MustVar::mask(var.name.clone(), &expr.span));
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
            // 条件分岐は`branch.rs`、反復処理は`loop_check.rs`で扱う
            ExprNode::Match { .. } => self.walk_match(expr, must_vars, fn_name),
            ExprNode::Loop { .. } => self.walk_loop(expr, must_vars, fn_name),
            // それ以外は子の式を順に辿る(`try_for_each_child`を`scan_loop_expr`と共有する)
            _ => Self::try_for_each_child(expr, &mut |child| {
                self.walk_expr(child, must_vars, fn_name)
            }),
        }
    }

    /// `&Box<Expr>`などを`&Expr`にそろえる(関数の引数での型強制を使う)
    pub(super) fn as_expr(expr: &Expr) -> &Expr {
        expr
    }

    /// 式の子の式を、評価する順に`f`へ渡す
    ///
    /// `walk_expr`と`scan_loop_expr`が同じ一覧を使うための入口。
    /// 契約の追跡で特別な扱いが要るノード(`DefVar`/`CallFunc`/`Match`/`Loop`)は
    /// 含めず、呼び出し側で先に処理する。式ノードを追加したときは、
    /// 子の式を持つなら、ここに足す(足さないと追跡が漏れる)
    pub(super) fn try_for_each_child<E>(
        expr: &Expr,
        f: &mut impl FnMut(&Expr) -> Result<(), E>,
    ) -> Result<(), E> {
        match &expr.node {
            ExprNode::Scope { target, .. }
            | ExprNode::Member { target, .. }
            | ExprNode::PtrMember { target, .. } => f(Self::as_expr(target)),
            ExprNode::Assign(assign) => {
                f(Self::as_expr(&assign.dst))?;
                f(Self::as_expr(&assign.value))
            }
            ExprNode::GetAddress(target) | ExprNode::ConnectAddr(target) => {
                f(Self::as_expr(target))
            }
            ExprNode::VariantBind(bind) => f(Self::as_expr(&bind.value)),
            ExprNode::Add(pair)
            | ExprNode::Sub(pair)
            | ExprNode::Mul(pair)
            | ExprNode::Div(pair)
            | ExprNode::Surplus(pair)
            | ExprNode::LessThen(pair)
            | ExprNode::GreaterThen(pair)
            | ExprNode::Equal(pair)
            | ExprNode::NotEq(pair) => {
                f(Self::as_expr(&pair.0))?;
                f(Self::as_expr(&pair.1))
            }
            ExprNode::InitStruct { fields, .. } => fields
                .values()
                .try_for_each(|val| f(Self::as_expr(val))),
            ExprNode::Array(values) => values.iter().try_for_each(|val| f(Self::as_expr(val))),
            ExprNode::RefArray { dst, index, .. } => {
                f(Self::as_expr(dst))?;
                f(Self::as_expr(index))
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
        // 最も内側の同名の要素だけを見る。それが契約を持たない変数なら何もしない
        if let Some(var) = must_vars.iter_mut().rev().find(|var| var.name == name) {
            if var.tracked {
                var.used = true;
            }
        }
    }
}
