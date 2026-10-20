//! 反復処理の契約チェック
//!
//! ループの外で定義した契約を、ループの中で終了するコードは
//! `ret`/`break`の前にしか置けない

use std::rc::Rc;

use super::*;
use crate::err::Span;

impl IR {
    /// 反復処理(`node::Expr::Loop`)の追跡
    /// (ループの中で定義した変数は、ループを抜けると追跡をやめる)
    pub(super) fn walk_loop(expr: &node::Expr, must_vars: &mut Vec<MustVar>) {
        let node::ExprNode::Loop {
            ref pattern,
            ref body,
        } = expr.node
        else {
            return;
        };
        let scope_start = must_vars.len();
        if let Some(pattern) = pattern {
            Self::walk_expr(&pattern, must_vars);
        }
        Self::walk_group2_nodes(&body, must_vars);
        must_vars.truncate(scope_start);
    }

    /// 関数全体を辿り、ループを見つけたら`check_loop_block`で検査する
    /// `scope`は今見えている`must`の変数名
    pub(super) fn check_loop_contract_ends(
        func_name: &String,
        nodes: &Body,
        scope: &mut Vec<String>,
    ) {
        for n in nodes.iter() {
            match n.get_node() {
                node::Group2Node::Expr(expr)
                | node::Group2Node::Stmt(node::StmtNode::Return(expr)) => {
                    Self::scan_loop_expr(func_name, &expr, scope);
                }
                _ => {}
            }
        }
    }

    fn scan_loop_expr(func_name: &String, expr: &node::Expr, scope: &mut Vec<String>) {
        match &expr.node {
            node::ExprNode::DefVar(var) => {
                Self::scan_loop_expr(func_name, &var.value, scope);
                if var.ty.is_constract_must() {
                    scope.push(var.name.clone());
                }
            }
            node::ExprNode::Match { arms, arm_else, .. } => {
                for arm in arms.iter() {
                    let start = scope.len();
                    Self::check_loop_contract_ends(func_name, &arm.body, scope);
                    scope.truncate(start);
                }
                if let Some(body) = arm_else {
                    let start = scope.len();
                    Self::check_loop_contract_ends(func_name, body, scope);
                    scope.truncate(start);
                }
            }
            node::ExprNode::Loop { body, .. } => {
                let mut guarded = scope.clone();
                Self::check_loop_block(func_name, body, &mut guarded);
            }
            _ => {}
        }
    }

    /// 式の中で終了される(関数の引数として渡される)`guarded`の変数名
    fn ended_by(expr: &node::Expr, guarded: &[String]) -> Vec<String> {
        let mut scratch: Vec<MustVar> = guarded.iter().cloned().map(MustVar::new).collect();
        Self::walk_expr(expr, &mut scratch);
        scratch
            .into_iter()
            .filter(|v| v.used)
            .map(|v| v.name)
            .collect()
    }

    /// ループ本体(や、その中の分岐の本体)の検査
    ///
    /// `guarded`はループの外で定義された`must`の変数。これらを終了する
    /// 文は、直後(他の終了する文を挟んでもよい)に`ret`/`break`が
    /// 無ければエラーにする。そうしないと次の周でもう一度終了してしまう。
    /// ループの中で定義した変数は毎周作り直されるので対象外
    fn check_loop_block(func_name: &String, body: &Body, guarded: &mut Vec<String>) {
        let mut locals: Vec<String> = Vec::new();
        // (この文が終了する変数, `ret`/`break`か, この文の位置)
        let mut stmts: Vec<(Vec<String>, bool, Rc<Span>)> = Vec::new();

        for n in body.iter() {
            let (expr, is_exit) = match n.get_node() {
                node::Group2Node::Expr(expr) => (Some(expr), false),
                node::Group2Node::Stmt(node::StmtNode::Return(expr)) => (Some(expr), true),
                node::Group2Node::Stmt(node::StmtNode::Break(_span)) => (None, true),
                _ => (None, false),
            };
            let mut ends: Vec<String> = Vec::new();
            if let Some(ref expr) = expr {
                match &expr.node {
                    // 分岐の本体は、それぞれ同じ規則で検査する
                    node::ExprNode::Match { arms, arm_else, .. } => {
                        for arm in arms.iter() {
                            Self::check_loop_block(func_name, &arm.body, &mut guarded.clone());
                        }
                        if let Some(body) = arm_else {
                            Self::check_loop_block(func_name, body, &mut guarded.clone());
                        }
                    }
                    // 入れ子のループは、自分の本体を検査する。
                    // さらに外側のループから見ると、中で終了することも終了する文
                    node::ExprNode::Loop { body, .. } => {
                        let mut inner = guarded.clone();
                        inner.extend(locals.iter().cloned());
                        Self::check_loop_block(func_name, body, &mut inner);
                        ends = Self::ended_by(expr, guarded);
                    }
                    _ => {
                        ends = Self::ended_by(expr, guarded);
                    }
                }
                // ループの中で定義した変数は、この周の中で終了するだけでよい
                if let node::ExprNode::DefVar(ref var) = expr.node {
                    if var.ty.is_constract_must() {
                        guarded.retain(|name| name != &var.name);
                        locals.push(var.name.clone());
                    }
                }
            }
            stmts.push((ends, is_exit, Rc::clone(&n.span)));
        }

        for (i, (ends, is_exit, span)) in stmts.iter().enumerate() {
            if ends.is_empty() || *is_exit {
                continue;
            }
            let mut placed_before_exit = false;
            for (next_ends, next_exit, _) in stmts.iter().skip(i + 1) {
                if *next_exit {
                    placed_before_exit = true;
                    break;
                }
                if next_ends.is_empty() {
                    break;
                }
            }
            if !placed_before_exit {
                for name in ends {
                    CompileErr::constract_must_not_used(func_name, name, Rc::clone(span)).unwrap();
                }
            }
        }
    }
}