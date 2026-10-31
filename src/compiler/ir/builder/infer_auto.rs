//! 型推論: `name := value`(`TyNode::Auto`)の型を決める
//!
//! 型は次の順で決める。
//!
//! 1. **使われ方からの推論**(`resolve_auto_by_usage`、関数の処理を始める前にASTへ反映)
//!    - `ret x`で返す変数は、関数の戻り値の型にする
//!    - `y: T = x` / `y = x`のように、型の分かる変数へ渡す変数は、その型にする
//!    - `a := b`のように変数同士が`:=`でつながっているときは、
//!      どちらかの型が決まるまで裏でたどる(固定点に達するまで繰り返す)
//! 2. **右辺からの推論**(`infer_auto_var_ty`、変数を定義するときに実行)
//!    - 変数 → 定義済みの変数の型(`a := b`は、`b`が`:=`で定義されていても、
//!      その時点で決まった型を引き継ぐ)
//!    - 関数・メゾットの呼び出し → 関数の戻り値の型(ジェネリクスは`temp_ty`ごと)
//!    - 構造体・バリアント型の初期化、`Name::Mem`、キャスト、`&x`、四則演算など
//! 3. どちらでも決まらなければエラー(`UndefVarTy`)
//!
//! ジェネリクスの関数・型は、IRに渡される時点で`temp_ty`ごとの本体に
//! 展開されているので、`T`は具体的な型になった状態で推論される。

use std::collections::HashMap;

use super::*;
use crate::compiler::ast::{ArgsNode, Expr, ExprNode, Group2Node, StmtNode, TyNode};
use crate::compiler::models::Body;

/// 名前 -> 型(`None`は`:=`で、まだ決まっていない)
type TyMap = HashMap<String, Option<TyNode>>;

// ---------------------------------------------------------------
// 1. 使われ方からの推論(ASTだけで完結するので、`IR`の状態を使わない)
// ---------------------------------------------------------------

/// 関数の本体の`:=`のうち、使われ方から型が決まるものを`TyNode::Auto`から
/// 決まった型へ書き換える。決まらなかったものは`Auto`のまま残す
/// (右辺からの推論`infer_auto_var_ty`に任せる)
pub(super) fn resolve_auto_by_usage(body: &mut Body, params: &[ArgsNode], ret_ty: &TyNode) {
    let mut known: TyMap = HashMap::new();
    for p in params {
        known.insert(p.name.clone(), Some(p.ty.clone()));
    }
    // 同じ名前を複数回定義している変数は、どの定義への使用か分からないので対象外にする
    let mut defined: HashMap<String, usize> = HashMap::new();
    collect_defs(body, &mut known, &mut defined);
    known.retain(|name, _| defined.get(name).copied().unwrap_or(1) <= 1);

    // 固定点に達するまで制約を伝える(`a := b`の連鎖を裏でたどる)
    loop {
        let mut changed = false;
        propagate(body, ret_ty, &mut known, &mut changed);
        if !changed {
            break;
        }
    }
    apply(body, &known);
}

fn collect_defs(body: &Body, known: &mut TyMap, defined: &mut HashMap<String, usize>) {
    for g in body.iter() {
        match &g.node {
            Group2Node::Expr(e) | Group2Node::Stmt(StmtNode::Return(e)) => {
                collect_defs_expr(e, known, defined)
            }
            _ => {}
        }
    }
}

fn collect_defs_expr(e: &Expr, known: &mut TyMap, defined: &mut HashMap<String, usize>) {
    match &e.node {
        ExprNode::DefVar(d) => {
            *defined.entry(d.name.clone()).or_insert(0) += 1;
            match d.ty {
                TyNode::Auto => {
                    known.entry(d.name.clone()).or_insert(None);
                }
                _ => {
                    known.insert(d.name.clone(), Some(d.ty.clone()));
                }
            }
            collect_defs_expr(&d.value, known, defined);
        }
        ExprNode::Match {
            pattern,
            arms,
            arm_else,
        } => {
            if let Some(p) = pattern {
                collect_defs_expr(p, known, defined);
            }
            for arm in arms.iter() {
                collect_defs(&arm.body, known, defined);
            }
            if let Some(b) = arm_else {
                collect_defs(b, known, defined);
            }
        }
        ExprNode::Loop { body, .. } => collect_defs(body, known, defined),
        _ => {}
    }
}

/// `name`の型が未決定(`:=`)なら`ty`で決める。変更があれば`changed`を立てる
fn set_if_unknown(known: &mut TyMap, name: &str, ty: &TyNode, changed: &mut bool) {
    if matches!(ty, TyNode::Auto) {
        return;
    }
    if let Some(slot) = known.get_mut(name) {
        if slot.is_none() {
            *slot = Some(ty.clone());
            *changed = true;
        }
    }
}

fn known_ty(known: &TyMap, name: &str) -> Option<TyNode> {
    known.get(name).cloned().flatten()
}

fn propagate(body: &Body, ret_ty: &TyNode, known: &mut TyMap, changed: &mut bool) {
    for g in body.iter() {
        match &g.node {
            // `ret x`: 戻り値に使う変数は、関数の戻り値の型
            Group2Node::Stmt(StmtNode::Return(e)) => {
                if let ExprNode::Var(x) = &e.node {
                    set_if_unknown(known, x, ret_ty, changed);
                }
            }
            Group2Node::Expr(e) => propagate_expr(e, ret_ty, known, changed),
            _ => {}
        }
    }
}

fn propagate_expr(e: &Expr, ret_ty: &TyNode, known: &mut TyMap, changed: &mut bool) {
    match &e.node {
        ExprNode::DefVar(d) => {
            if let ExprNode::Var(src) = &d.value.node {
                match &d.ty {
                    // `y: T = x`: `x`は`T`
                    ty if !matches!(ty, TyNode::Auto) => set_if_unknown(known, src, ty, changed),
                    // `y := x`: どちらかが決まれば、もう片方も同じ型
                    _ => {
                        if let Some(t) = known_ty(known, src) {
                            set_if_unknown(known, &d.name, &t, changed);
                        } else if let Some(t) = known_ty(known, &d.name) {
                            set_if_unknown(known, src, &t, changed);
                        }
                    }
                }
            }
            propagate_expr(&d.value, ret_ty, known, changed);
        }
        // `y = x`: `x`は`y`と同じ型
        ExprNode::Assign(a) => {
            if let ExprNode::Var(src) = &a.value.node {
                if let Some(t) = known_ty(known, &a.name) {
                    set_if_unknown(known, src, &t, changed);
                }
            }
            // 代入先が`:=`で未決定なら、右辺の型を引き継ぐ
            if let ExprNode::Var(src) = &a.value.node {
                if let Some(t) = known_ty(known, src) {
                    set_if_unknown(known, &a.name, &t, changed);
                }
            }
        }
        ExprNode::Match {
            pattern,
            arms,
            arm_else,
        } => {
            if let Some(p) = pattern {
                propagate_expr(p, ret_ty, known, changed);
            }
            for arm in arms.iter() {
                propagate(&arm.body, ret_ty, known, changed);
            }
            if let Some(b) = arm_else {
                propagate(b, ret_ty, known, changed);
            }
        }
        ExprNode::Loop { body, .. } => propagate(body, ret_ty, known, changed),
        _ => {}
    }
}

fn apply(body: &mut Body, known: &TyMap) {
    for g in body.iter_mut() {
        match &mut g.node {
            Group2Node::Expr(e) | Group2Node::Stmt(StmtNode::Return(e)) => apply_expr(e, known),
            _ => {}
        }
    }
}

fn apply_expr(e: &mut Expr, known: &TyMap) {
    match &mut e.node {
        ExprNode::DefVar(d) => {
            if matches!(d.ty, TyNode::Auto) {
                if let Some(Some(t)) = known.get(&d.name) {
                    d.ty = t.clone();
                }
            }
            apply_expr(&mut d.value, known);
        }
        ExprNode::Match {
            pattern,
            arms,
            arm_else,
        } => {
            if let Some(p) = pattern {
                apply_expr(p, known);
            }
            for arm in arms.iter_mut() {
                apply(&mut arm.body, known);
            }
            if let Some(b) = arm_else {
                apply(b, known);
            }
        }
        ExprNode::Loop { body, .. } => apply(body, known),
        _ => {}
    }
}

// ---------------------------------------------------------------
// 2. 右辺からの推論(定義済みの変数・関数・型の情報を使う)
// ---------------------------------------------------------------

impl IR {
    /// `var.ty`が`TyNode::Auto`なら、右辺から型を決めて返す。
    /// `Auto`でなければ、そのまま返す。決まらなければエラー
    pub(super) fn resolve_auto_var_ty(&self, var: &ast::DefineVar) -> Result<TyNode, err::ErrKind> {
        if !matches!(var.ty, TyNode::Auto) {
            return Ok(var.ty.clone());
        }
        self.infer_ty_node(&var.value).ok_or_else(|| {
            // 即値(`x := 10`)のように、右辺だけでは型が決まらず、
            // 使われ方からも決まらなかった変数
            crate::GenUndefErrResult!(UndefVarTy, var.name.to_string(), None)
        })
    }

    /// 式の型を`TyNode`で返す。決まらなければ`None`
    ///
    /// 副作用が無い(変数の使用回数も数えない)ので`&self`で調べる。
    /// 型が決まっていない即値(`10`・`1.5`)は`None`
    pub(super) fn infer_ty_node(&self, expr: &Expr) -> Option<TyNode> {
        let ty = match &expr.node {
            // 変数: 定義済みの型(`:=`で定義された変数も、定義した時点で決まっている)
            ExprNode::Var(name) => self.var_tree.get_ty_node(name)?,
            ExprNode::Cast((ty, _)) => ty.clone(),
            // 構造体・バリアント型の初期化: その型
            ExprNode::InitStruct { name, .. } | ExprNode::InitVariant { name, .. } => {
                TyNode::Ty(name.clone())
            }
            // `Name::Mem`は、バリアント型(型なしメンバーを含む)の値
            ExprNode::EnumVariant { name, .. } => TyNode::Ty(name.clone()),
            // 関数の呼び出し: 関数の戻り値の型
            ExprNode::CallFunc(call) => {
                self.func_tree
                    .get_with_temp(&call.name, None, &call.temp_ty)?
                    .ret_ty?
            }
            // `Type::func(..)` / `mod::func(..)`
            ExprNode::Scope { scope, target } => match &target.node {
                ExprNode::CallFunc(call) => {
                    // `Type::Mem(value)`がバリアント型の初期化なら、その型
                    if let Expr {
                        node: ExprNode::InitVariant { name, .. },
                        ..
                    } = self.scope_call_to_variant_init(expr)
                    {
                        return Some(TyNode::Ty(name));
                    }
                    let module = scope.last().map(|s| s.as_str());
                    self.func_tree
                        .get_with_temp(&call.name, module, &call.temp_ty)?
                        .ret_ty?
                }
                _ => return None,
            },
            ExprNode::GetAddress(inner) => TyNode::Pointer {
                is_const: false,
                ty_name: Box::new(self.infer_ty_node(inner)?),
                range: None,
            },
            // 単項演算の結果は、オペランドと同じ型
            ExprNode::Sign((_, a))
            | ExprNode::Inversion(a)
            | ExprNode::Inc(a)
            | ExprNode::Dec(a)
            | ExprNode::LeftInc(a)
            | ExprNode::LeftDec(a) => return self.infer_ty_node(a),
            // 四則演算・ビット演算: 型が分かるほうのオペランドの型
            ExprNode::Add(n)
            | ExprNode::Sub(n)
            | ExprNode::Mul(n)
            | ExprNode::Div(n)
            | ExprNode::Surplus(n)
            | ExprNode::LShift(n)
            | ExprNode::RShift(n)
            | ExprNode::AndCalcul(n)
            | ExprNode::XorCalcul(n)
            | ExprNode::OrCalcul(n) => self
                .infer_ty_node(&n.0)
                .or_else(|| self.infer_ty_node(&n.1))?,
            // 比較・論理演算の結果は真偽値
            ExprNode::LessThen(_)
            | ExprNode::GreaterThen(_)
            | ExprNode::LessThenEq(_)
            | ExprNode::GreaterThenEq(_)
            | ExprNode::Equal(_)
            | ExprNode::NotEq(_)
            | ExprNode::CmpAnd(_)
            | ExprNode::CmpOr(_)
            | ExprNode::CmpNot(_) => TyNode::Ty("bool".to_string()),
            ExprNode::Ternary((_, a, b)) => {
                self.infer_ty_node(a).or_else(|| self.infer_ty_node(b))?
            }
            _ => return None,
        };
        (!matches!(ty, TyNode::Auto)).then_some(ty)
    }
}
