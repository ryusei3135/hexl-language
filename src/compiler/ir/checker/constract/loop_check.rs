//! 反復処理の契約チェック
//!
//! ループの外で定義した契約を、ループの中で終了するコードは
//! `ret`/`break`の前にしか置けない

use std::rc::Rc;

use super::*;
use crate::compiler::ast::{Expr, ExprNode, Group2Node, StmtNode};
use crate::compiler::ir::def_tree::VarLife::EndConstract;
use crate::err::{self, ErrKind, Span, compile::{self, CompileErrKind::ConstractUnterminableScope}};

impl IR {
    /// 反復処理(`Expr::Loop`)の追跡
    /// (ループの中で定義した変数は、ループを抜けると追跡をやめる)
    pub(super) fn walk_loop(
        &mut self,
        expr: &Expr,
        must_vars: &MustVarsTy,
        fn_name: &str,
    ) -> Result<Vec<String>, err::ErrKind> {
        let ExprNode::Loop {
            ref pattern,
            ref body,
        } = expr.node
        else {
            return Ok(Vec::new());
        };
        let scope_start = must_vars.borrow().len();
        let mut ends: Vec<String> = Vec::new();
        if let Some(pattern) = pattern {
            Self::push_ends(&mut ends, self.walk_expr(&pattern, must_vars, fn_name)?);
        }
        Self::push_ends(&mut ends, self.walk_group2_nodes(&body, must_vars, fn_name)?);
        // Loopの中で新たに契約したのにloopで契約を終了していない場合エラー
        self.emit_scope_constract_errs(scope_start, must_vars, fn_name);

        // ループの中で定義した変数は外側から見えないので、終了した契約から除く
        {
            let vars = must_vars.borrow();
            let outer: Vec<&str> = vars[..scope_start.min(vars.len())]
                .iter()
                .filter(|v| v.tracked)
                .map(|v| v.name.as_str())
                .collect();
            ends.retain(|name| outer.contains(&name.as_str()));
        }
        must_vars.borrow_mut().truncate(scope_start);
        Ok(ends)
    }

    /// 関数全体を辿り、ループを見つけたら`check_loop_block`で検査する
    /// `scope`は今見えている`must`の変数名
    pub(super) fn check_loop_contract_ends(
        &mut self,
        fn_name: &str,
        nodes: &Body,
        scope: &MustVarsTy,
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
    pub(super) fn emit_scope_constract_errs(
        &mut self,
        start: usize,
        must_vars: &MustVarsTy,
        fn_name: &str,
    ) {
        // Loopの中で新たに契約したのにloopで契約を終了していない場合エラー
        // (`RefCell`の借用を持ったままエラープールに積まないよう、先に集める)
        let unused: Vec<(String, Rc<Span>)> = must_vars
            .borrow()
            .iter()
            .skip(start)
            // 複数のエラーをスタック(契約を終了していない変数だけ)
            .filter(|var| var.tracked && !var.used)
            .map(|var| (var.name.clone(), Rc::clone(&var.span)))
            .collect();
        for (var_name, span) in unused {
            let result: Result<(), err::ErrKind> = crate::GenCompileErr2!(
                CompileErrKind::ContractMustNotUsed {
                    fn_name: fn_name.to_string(),
                    var_name
                },
                span
            );
            (*self.err_pool.borrow_mut()).push_err_node(result.unwrap_err());
        }
    }

    /// 反復処理ではなく関数のノードの中を見る
    ///
    /// ExprNode::Matchで契約のチェック
    fn scan_loop_expr(
        &mut self,
        fn_name: &str,
        expr: &Expr,
        scope: &MustVarsTy,
    ) -> Result<(), err::ErrKind> {
        match &expr.node {
            ExprNode::DefVar(var) => {
                self.scan_loop_expr(fn_name, &var.value, scope)?;
                if var.ty.is_constract_must() {
                    scope
                        .borrow_mut()
                        .push(MustVar::new(var.name.clone(), &expr.span));
                } else {
                    let shadows = scope.borrow().iter().any(|entry| entry.name == var.name);
                    if shadows {
                        scope
                            .borrow_mut()
                            .push(MustVar::mask(var.name.clone(), &expr.span));
                    }
                }
            }
            ExprNode::Match { .. } => {
                self.tracking_cond_expr_node(expr, scope, fn_name)?;
            }
            ExprNode::Loop { body, .. } => {
                let guarded = Self::fork_vars(scope);
                self.check_loop_block(fn_name, body, &guarded)?;
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
    pub(super) fn scan_loop_branch(
        &mut self,
        fn_name: &str,
        pattern: Option<&Expr>,
        body: &Body,
        outer: &MustVarsTy,
    ) -> Result<Vec<MustVar>, err::ErrKind> {
        let outer_len = outer.borrow().len();
        let local = Self::fork_vars(outer);
        if let Some(pattern) = pattern {
            self.scan_loop_expr(fn_name, pattern, &local)?;
        }
        self.check_loop_contract_ends(fn_name, body, &local)?;
        self.emit_scope_constract_errs(outer_len, &local, fn_name);
        let mut state = local.borrow().clone();
        state.truncate(outer_len);
        Ok(state)
    }

    /// ループ本体(や、その中の分岐の本体)の検査
    ///
    /// `guarded`はループの外で定義された`must`の変数。これらを終了する
    /// 文は、直後(他の終了する文を挟んでもよい)に`ret`/`break`が
    /// 無ければエラーにする。そうしないと次の周でもう一度終了してしまう。
    /// ループの中で定義した変数は毎周作り直されるので対象外
    pub(super) fn check_loop_block(
        &mut self,
        fn_name: &str,
        body: &Body,
        guarded: &MustVarsTy,
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
                    // 分岐の本体は、それぞれ同じ規則で検査する(`cond_expr_tracking.rs`)
                    ExprNode::Match { .. } => {
                        ends = self.tracking_cond_expr_node(expr, guarded, fn_name)?;
                    }
                    // 入れ子のループは、自分の本体を検査する。
                    // さらに外側のループから見ると、中で終了することも終了する文
                    ExprNode::Loop { body, .. } => {
                        let inner = Self::fork_vars(guarded);
                        inner.borrow_mut().extend(locals.iter().cloned());
                        self.check_loop_block(fn_name, body, &inner)?;
                        ends = self.ended_by(expr, guarded, fn_name)?;
                    }
                    _ => {
                        ends = self.ended_by(expr, guarded, fn_name)?;
                    }
                }
                // ループの中で定義した変数は、この周の中で終了するだけでよい
                // (契約のない変数が外側の契約変数を隠す場合も、以降は外側の変数を見ない)
                if let ExprNode::DefVar(ref var) = expr.node {
                    guarded.borrow_mut().retain(|v| v.name != var.name);
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
