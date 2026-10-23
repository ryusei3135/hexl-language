//! 反復処理の契約チェック
//!
//! ループの外で定義した契約を、ループの中で終了するコードは
//! `ret`/`break`の前にしか置けない

use std::rc::Rc;

use super::*;
use crate::compiler::ast::{Expr, ExprNode, Group2Node, StmtNode};
use crate::compiler::ir::def_tree::VarLife::EndConstract;
use crate::err::{self, ErrKind, Span, compile::*};

impl IR {
    /// 反復処理(`Expr::Loop`)の追跡
    /// (ループの中で定義した変数は、ループを抜けると追跡をやめる)
    pub(super) fn walk_loop(
        &mut self,
        expr: &Expr,
        must_vars: &mut Vec<MustVar>,
        fn_name: &str,
    ) -> Result<(), err::ErrKind> {
        let ExprNode::Loop {
            ref pattern,
            ref body,
        } = expr.node
        else {
            return Ok(());
        };
        let scope_start = must_vars.len();
        if let Some(pattern) = pattern {
            self.walk_expr(&pattern, must_vars, fn_name)?;
        }
        self.walk_group2_nodes(&body, must_vars, fn_name)?;
        // Loopの中で新たに契約したのにloopで契約を終了していない場合エラー
        self.emit_scope_constract_errs(scope_start, must_vars, fn_name);

        must_vars.truncate(scope_start);
        Ok(())
    }

    /// 関数全体を辿り、ループを見つけたら`check_loop_block`で検査する
    /// `scope`は今見えている`must`の変数名
    pub(super) fn check_loop_contract_ends(
        &mut self,
        fn_name: &str,
        nodes: &Body,
        scope: &mut Vec<MustVar>,
    ) -> Result<(), err::ErrKind> {
        for n in nodes.iter() {
            match n.get_node() {
                Group2Node::Expr(expr) | Group2Node::Stmt(StmtNode::Return(expr)) => {
                    self.scan_loop_expr(fn_name, &expr, scope)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// スコープが終了したときスコープで契約が終了していなかったら
    /// エラープールにすべてのエラーをスタック
    fn emit_scope_constract_errs(
        &mut self,
        start: usize,
        must_vars: &mut Vec<MustVar>,
        fn_name: &str,
    ) {
        // Loopの中で新たに契約したのにloopで契約を終了していない場合エラー
        if must_vars.len() > start {
            // 複数のエラーをスタック
            for i in (start - 1)..must_vars.len() {
                let result: Result<(), err::ErrKind> = crate::GenCompileErr2!(
                    CompileErrKind::ContractMustNotUsed {
                        fn_name: fn_name.to_string(),
                        var_name: must_vars[i].name.clone()
                    },
                    Rc::clone(&must_vars[i].span)
                );
                (*self.err_pool.borrow_mut()).push_err_node(result.unwrap_err());
            }
        }
    }

    /// 反復処理ではなく関数のノードの中を見る
    ///
    /// ExprNode::Matchで契約のチェック
    fn scan_loop_expr(
        &mut self,
        fn_name: &str,
        expr: &Expr,
        scope: &mut Vec<MustVar>,
    ) -> Result<(), err::ErrKind> {
        match &expr.node {
            ExprNode::DefVar(var) => {
                self.scan_loop_expr(fn_name, &var.value, scope)?;
                if var.ty.is_constract_must() {
                    scope.push(MustVar::new(var.name.clone(), &expr.span));
                }
            }
            ExprNode::Match { arms, arm_else, .. } => {
                // 1. Matchに入る前の未終了リスト（クローンして保持）
                let mut common_musts: HashSet<_> = scope.iter().cloned().collect();
                let mut visited_any_arm = false;

                for arm in arms.iter() {
                    let start = scope.len();
                    self.check_loop_contract_ends(fn_name, &arm.body, scope)?;
                    self.emit_scope_constract_errs(start, scope, fn_name);

                    // このアームの処理が終わった時点のscopeの要素をSetにする
                    let current_musts: HashSet<_> = scope[start..].iter().cloned().collect();

                    // 最初のアームならそのままコピー、2回目以降は「積集合（両方に残っているもの）」を取る
                    if !visited_any_arm {
                        common_musts = current_musts;
                        visited_any_arm = true;
                    } else {
                        common_musts = common_musts.intersection(&current_musts).cloned().collect();
                    }

                    scope.truncate(start);
                }
                // elseの部分
                if let Some(body) = arm_else {
                    let start = scope.len();
                    self.check_loop_contract_ends(fn_name, body, scope)?;
                    self.emit_scope_constract_errs(start, scope, fn_name);

                    let current_musts: HashSet<_> = scope[start..].iter().cloned().collect();
                    if !visited_any_arm {
                        common_musts = current_musts;
                    } else {
                        common_musts = common_musts.intersection(&current_musts).cloned().collect();
                    }
                    scope.truncate(start);
                }
                // 2. すべての分岐を通り抜けた後、共通して残ってしまった契約をscopeに戻す
                scope.extend(common_musts);
            }
            ExprNode::Loop { body, .. } => {
                let mut guarded = scope.clone();
                self.check_loop_block(&fn_name, body, &mut guarded)?;
            }
            // 関数の呼び出し: 引数に渡された`must`の変数を消費済みにする
            ExprNode::CallFunc(call_info) => {
                let f = |arg: &Expr| -> Result<(), err::ErrKind> {
                    if let Some(name) = Self::arg_var_name(arg) {
                        Self::mark_used(name, scope);
                    }
                    self.walk_expr(arg, scope, fn_name)?;
                    Ok(())
                };
                // 💡 for_each から try_for_each に変更し、末尾に `?` をつける
                call_info.args.iter().try_for_each(f)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// 式の中で終了される(関数の引数として渡される)`guarded`の変数名
    fn ended_by(
        &mut self,
        expr: &Expr,
        guarded: &mut Vec<MustVar>,
        fn_name: &str,
    ) -> Result<Vec<String>, err::ErrKind> {
        self.walk_expr(expr, guarded, fn_name)?;
        Ok(guarded
            .into_iter()
            .filter(|v| v.used)
            .map(|v| v.name.clone())
            .collect())
    }

    /// ループ本体(や、その中の分岐の本体)の検査
    ///
    /// `guarded`はループの外で定義された`must`の変数。これらを終了する
    /// 文は、直後(他の終了する文を挟んでもよい)に`ret`/`break`が
    /// 無ければエラーにする。そうしないと次の周でもう一度終了してしまう。
    /// ループの中で定義した変数は毎周作り直されるので対象外
    fn check_loop_block(
        &mut self,
        fn_name: &str,
        body: &Body,
        guarded: &mut Vec<MustVar>,
    ) -> Result<(), err::ErrKind> {
        let mut locals: Vec<MustVar> = Vec::new();
        // (この文が終了する変数, `ret`/`break`か, この文の位置)
        let mut stmts: Vec<(Vec<String>, bool, Rc<Span>)> = Vec::new();

        for n in body.iter() {
            let (expr, is_exit) = match n.get_node() {
                Group2Node::Expr(expr) => (Some(expr), false),
                Group2Node::Stmt(StmtNode::Return(expr)) => (Some(expr), true),
                Group2Node::Stmt(StmtNode::Break(_span)) => (None, true),
                _ => (None, false),
            };
            let mut ends: Vec<String> = Vec::new();
            if let Some(ref expr) = expr {
                match &expr.node {
                    // 分岐の本体は、それぞれ同じ規則で検査する
                    ExprNode::Match { arms, arm_else, .. } => {
                        for arm in arms.iter() {
                            self.check_loop_block(fn_name, &arm.body, &mut guarded.clone())?;
                        }
                        if let Some(body) = arm_else {
                            self.check_loop_block(fn_name, body, &mut guarded.clone())?;
                        }
                    }
                    // 入れ子のループは、自分の本体を検査する。
                    // さらに外側のループから見ると、中で終了することも終了する文
                    ExprNode::Loop { body, .. } => {
                        let mut inner = guarded.clone();
                        inner.extend(locals.iter().cloned());
                        self.check_loop_block(fn_name, body, &mut inner)?;
                        ends = self.ended_by(expr, guarded, fn_name)?;
                    }
                    _ => {
                        ends = self.ended_by(expr, guarded, fn_name)?;
                    }
                }
                // ループの中で定義した変数は、この周の中で終了するだけでよい
                if let ExprNode::DefVar(ref var) = expr.node {
                    if var.ty.is_constract_must() {
                        guarded.retain(|v| v.name != var.name);
                        locals.push(MustVar::new(var.name.clone(), &expr.span));
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
                    return CompileErr::constract_must_not_used(fn_name, name, Rc::clone(span));
                }
            }
        }
        Ok(())
    }
}
