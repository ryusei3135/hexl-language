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
            // 複数のエラーをスタック(契約を終了していない変数だけ)
            for i in start..must_vars.len() {
                if !must_vars[i].tracked || must_vars[i].used {
                    continue;
                }
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
                } else if scope.iter().any(|entry| entry.name == var.name) {
                    scope.push(MustVar::mask(var.name.clone(), &expr.span));
                }
            }
            ExprNode::Match {
                pattern,
                arms,
                arm_else,
            } => {
                // 分岐の式(`cond`の条件など)は分岐の前に評価されるので、
                // 分岐前の`scope`に対して走査する
                if let Some(pattern) = pattern {
                    self.scan_loop_expr(fn_name, Self::as_expr(pattern), scope)?;
                }
                // 分岐の前の`scope`(外側の変数)は、どの分岐の後でも残す。
                // 各分岐は分岐前の状態のコピーに対して解析し、分岐の中で
                // 定義した変数は外側の変数とは別に扱って、分岐の終わりで捨てる。
                let mut branches: Vec<Vec<MustVar>> = Vec::new();
                for arm in arms.iter() {
                    // 各アームのパターン式は、そのアームの状態の中で評価される
                    branches.push(self.scan_loop_branch(
                        fn_name,
                        Some(Self::as_expr(&arm.pattern)),
                        &arm.body,
                        scope,
                    )?);
                }
                if let Some(body) = arm_else {
                    branches.push(self.scan_loop_branch(fn_name, None, body, scope)?);
                }
                // 外側の変数ごとに分岐の結果を合流させる。
                // どれか一つの分岐でも終了していれば、その先では終了済みの
                // 可能性があるものとして扱う(一部の分岐でしか終了しない場合の
                // エラーは、`walk_match`が報告する)
                for (i, var) in scope.iter_mut().enumerate() {
                    if var.tracked && branches.iter().any(|branch| branch[i].used) {
                        var.used = true;
                    }
                }
            }
            ExprNode::Loop { body, .. } => {
                let mut guarded = scope.clone();
                self.check_loop_block(&fn_name, body, &mut guarded)?;
            }
            // 関数の呼び出し: 引数に渡された`must`の変数を消費済みにする
            // (引数の中にも反復処理や分岐があり得るので、引数も同じ走査で辿る)
            ExprNode::CallFunc(call_info) => {
                for arg in call_info.args.iter() {
                    if let Some(name) = Self::arg_var_name(arg) {
                        Self::mark_used(name, scope);
                    }
                    self.scan_loop_expr(fn_name, arg, scope)?;
                }
            }
            // 代入・演算・配列や構造体の初期化など、それ以外の式は子の式を辿る。
            // (ここで辿らないと、式の奥にある反復処理や終了を見落とす)
            _ => Self::try_for_each_child(expr, &mut |child| {
                self.scan_loop_expr(fn_name, child, scope)
            })?,
        }
        Ok(())
    }

    /// 分岐1つ分を、分岐前の`scope`のコピーに対して解析して返す
    /// (分岐の中で定義された変数は、検査したうえで結果に含めない)
    fn scan_loop_branch(
        &mut self,
        fn_name: &str,
        pattern: Option<&Expr>,
        body: &Body,
        outer: &[MustVar],
    ) -> Result<Vec<MustVar>, err::ErrKind> {
        let outer_len = outer.len();
        let mut local = outer.to_vec();
        if let Some(pattern) = pattern {
            self.scan_loop_expr(fn_name, pattern, &mut local)?;
        }
        self.check_loop_contract_ends(fn_name, body, &mut local)?;
        self.emit_scope_constract_errs(outer_len, &mut local, fn_name);
        local.truncate(outer_len);
        Ok(local)
    }

    /// 式の中で終了される(関数の引数として渡される)`guarded`の変数名
    fn ended_by(
        &mut self,
        expr: &Expr,
        guarded: &mut Vec<MustVar>,
        fn_name: &str,
    ) -> Result<Vec<String>, err::ErrKind> {
        // 式を辿る前から終了済みだった変数は、この式で終了したことにしない
        // (前の文で終了した変数が、あとの文すべてで終了したことになってしまう)
        let used_before: Vec<bool> = guarded.iter().map(|v| v.used).collect();
        self.walk_expr(expr, guarded, fn_name)?;
        Ok(guarded
            .iter()
            .zip(used_before)
            .filter(|(v, before)| v.tracked && v.used && !before)
            .map(|(v, _)| v.name.clone())
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
                // 全経路が`ret`/`break`で抜ける分岐も、脱出する文として扱う
                Group2Node::Expr(expr) => {
                    let is_exit = Self::expr_exits(&expr).is_some();
                    (Some(expr), is_exit)
                }
                Group2Node::Stmt(StmtNode::Return(expr)) => (Some(expr), true),
                Group2Node::Stmt(StmtNode::Break(_span)) => (None, true),
                _ => (None, false),
            };
            let mut ends: Vec<String> = Vec::new();
            if let Some(ref expr) = expr {
                match &expr.node {
                    // 分岐の本体は、それぞれ同じ規則で検査する
                    ExprNode::Match {
                        pattern,
                        arms,
                        arm_else,
                    } => {
                        // 分岐の式(`cond`の条件など)で終了するなら、この文で終了する
                        if let Some(pattern) = pattern {
                            ends = self.ended_by(Self::as_expr(pattern), guarded, fn_name)?;
                        }
                        for arm in arms.iter() {
                            // アームのパターン式で終了するなら、この文で終了する
                            for name in
                                self.ended_by(Self::as_expr(&arm.pattern), &mut guarded.clone(), fn_name)?
                            {
                                if !ends.contains(&name) {
                                    ends.push(name);
                                }
                            }
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
                // (契約のない変数が外側の契約変数を隠す場合も、以降は外側の変数を見ない)
                if let ExprNode::DefVar(ref var) = expr.node {
                    guarded.retain(|v| v.name != var.name);
                    if var.ty.is_constract_must() {
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
