//! 反復処理の契約チェック
//!
//! ループの外で定義した契約を、ループの中で終了するコードは
//! `ret`/`break`の前にしか置けない

use std::rc::Rc;

use super::*;
use crate::compiler::ast::{Expr, ExprNode, Group2Node, StmtNode, stmt_node::MatchArm};
use crate::compiler::ir::def_tree::VarLife::EndConstract;
use crate::err::{
    self, ErrKind, Span,
    compile::{self, CompileErrKind::ConstractUnterminableScope},
};

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
            let found = self.walk_expr(&pattern, must_vars, fn_name, true)?;
            Self::push_ends(&mut ends, found);
        }
        Self::push_ends(
            &mut ends,
            self.walk_group2_nodes(&body, must_vars, fn_name, true)?,
        );
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
            ExprNode::Match {
                pattern,
                arms,
                arm_else,
            } => {
                // ここはループの外。分岐の中にあるループを探すだけで、
                // ループ専用の制約(`tracking_cond_expr_node`)は使わない。
                // 分岐の式(`cond`の条件など)は分岐の前に評価されるので、
                // 分岐前の`scope`に対して走査する
                self.scan_cond_in_loop(pattern.as_deref(), &arms, &arm_else, scope, fn_name)?;
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

    /// 条件分岐の中にある反復処理を探す関数
    #[inline(always)]
    fn scan_cond_in_loop(
        &mut self,
        pattern: Option<&Expr>,
        arms: &[MatchArm],
        arm_else: &Option<Body>,
        scope: &MustVarsTy,
        fn_name: &str,
    ) -> Result<(), err::ErrKind> {
        // ここはループの外。分岐の中にあるループを探すだけで、
        // ループ専用の制約(`tracking_cond_expr_node`)は使わない。
        // 分岐の式(`cond`の条件など)は分岐の前に評価されるので、
        // 分岐前の`scope`に対して走査する
        if let Some(pattern) = pattern {
            self.scan_loop_expr(fn_name, Self::as_expr(pattern), scope)?;
        }
        // 各分岐は分岐前の状態のコピーに対して解析し、分岐の中で
        // 定義した変数は外側の変数とは別に扱って、分岐の終わりで捨てる
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
        scope
            .borrow_mut()
            .iter_mut()
            .enumerate()
            .filter(|(i, var)| var.tracked && branches.iter().any(|branch| branch[*i].used))
            .for_each(|var| var.1.used = true);
        Ok(())
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
        // この関数が`guarded`に積んだ要素(マスクなど)は、終わりに取り除く
        let start_len = guarded.borrow().len();
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
                    // 分岐の本体は、それぞれ同じ規則で検査する(`branch.rs`の`tracking_cond_expr_node`)
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
                // ループの中で定義した変数は、この周の中で終了するだけでよい。
                // 外側の同名の契約変数は消さず、内側の変数がそれを隠している間だけ
                // 見えなくする(マスクを積む。積んだ分は関数の終わりで取り除く)
                if let ExprNode::DefVar(ref var) = expr.node {
                    {
                        let mut g = guarded.borrow_mut();
                        if var.ty.is_constract_must() {
                            // `ended_by`(`walk_expr`)が積んだ、この変数自身の要素は検査の対象外にする
                            if let Some(own) = g
                                .iter_mut()
                                .rev()
                                .find(|v| v.name == var.name && Rc::ptr_eq(&v.span, &expr.span))
                            {
                                own.tracked = false;
                            }
                        } else if g
                            .iter()
                            .rev()
                            .find(|v| v.name == var.name)
                            .is_some_and(|v| v.tracked)
                        {
                            // 契約のない変数が、外側の契約変数を隠す
                            g.push(MustVar::mask(var.name.clone(), &expr.span));
                        }
                    }
                    // この周で定義した契約変数(入れ子のループへ引き継ぐ)。
                    // 同名の変数を定義し直したら、前の変数はマスクで隠す
                    if var.ty.is_constract_must() {
                        locals.push(MustVar::new(var.name.clone(), &expr.span));
                    } else if locals.iter().any(|v| v.name == var.name) {
                        locals.push(MustVar::mask(var.name.clone(), &expr.span));
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
        // 内側のスコープで積んだ要素だけを取り除く(外側の契約変数は残る)
        guarded.borrow_mut().truncate(start_len);
        Ok(())
    }
}
