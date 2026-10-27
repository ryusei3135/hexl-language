//! 契約の追跡で、条件分岐・反復処理に共通する処理
//! (`must`の変数の追跡状態と、式を辿る処理)

use super::*;
use crate::compiler::{
    ExprNode::Number,
    ast::{Expr, ExprNode, Group2Node, StmtNode},
};

/// 契約を持つ変数の一覧(共有して、あとから状態を書き換える)
pub(super) type MustVarsTy = Rc<RefCell<Vec<MustVar>>>;

/// 関数の中で追跡している、`must`の契約を持つ変数
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct MustVar {
    pub(super) name: String,
    /// 一度でも関数の引数として渡されたら`true`
    pub(super) used: bool,
    /// 条件分岐の一部の分岐でしか契約を終了していなければ`true`
    /// (すべての分岐で終了する必要があるのでエラー)
    pub(super) branch_mismatch: bool,
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

/// 契約を終了するメゾットの名前(`must`のフィールドを持つ構造体の、デストラクタ)
const CONTRACT_END_METHOD: &str = "del";

impl IR {
    /// 型が、契約を持つか
    ///
    /// - `T must=f`
    /// - `must`のフィールドを持つ構造体`A`(フィールドの契約は、構造体を
    ///   初期化したスコープの中で`a.del()`により終了すればよい)
    pub(super) fn owns_contract(&self, ty: &TyNode) -> bool {
        if ty.is_contract_must() {
            return true;
        }
        match ty {
            TyNode::Ty(name) => self
                .struct_tree
                .get(name)
                .is_some_and(|def| def.fields.iter().any(|f| f.ty.is_contract_must())),
            _ => false,
        }
    }

    /// `変数.del()`のような、構造体のフィールドの契約を終了するメゾット呼び出しか
    pub(super) fn is_contract_end_call(target: &Expr) -> bool {
        matches!(&target.node, ExprNode::CallFunc(call) if call.name == CONTRACT_END_METHOD)
    }

    /// 分岐の解析用に、変数の状態を複製する
    /// (`Rc`の`clone`は同じ一覧を共有してしまうので、中身を複製した別の一覧を作る)
    pub(super) fn fork_vars(must_vars: &MustVarsTy) -> MustVarsTy {
        Rc::new(RefCell::new(must_vars.borrow().clone()))
    }

    /// 終了した契約(この呼び出しで新しく終了した変数名)を、重複なく`dst`へ足す
    pub(super) fn push_ends(dst: &mut Vec<String>, src: Vec<String>) {
        for name in src {
            if !dst.contains(&name) {
                dst.push(name);
            }
        }
    }

    /// 文・式の列を辿り、この中で新しく終了した契約の変数名を返す
    pub(super) fn walk_group2_nodes(
        &mut self,
        nodes: &Body,
        must_vars: &MustVarsTy,
        fn_name: &str,
        in_loop: bool,
    ) -> Result<Vec<String>, err::ErrKind> {
        let mut ends: Vec<String> = Vec::new();
        for n in nodes.iter() {
            match n.get_node() {
                Group2Node::Expr(expr) => {
                    let found = self.walk_expr(&expr, must_vars, fn_name, in_loop)?;
                    Self::push_ends(&mut ends, found);
                }
                Group2Node::Stmt(StmtNode::Return(expr)) => {
                    let found = self.walk_expr(&expr, must_vars, fn_name, in_loop)?;
                    Self::push_ends(&mut ends, found);
                }
                Group2Node::Stmt(StmtNode::Continue | StmtNode::Break(..)) => {}
                _ => {}
            }
        }
        Ok(ends)
    }

    /// 式を辿り、この式の中で新しく終了した(`used`になった)契約の変数名を返す
    ///
    /// `in_loop`は反復処理の中か(条件分岐が`break`で抜けてよいかの判断に使う)
    pub(super) fn walk_expr(
        &mut self,
        expr: &Expr,
        must_vars: &MustVarsTy,
        fn_name: &str,
        in_loop: bool,
    ) -> Result<Vec<String>, err::ErrKind> {
        match &expr.node {
            // 変数の定義: `must`の型ならここから追跡を始める
            ExprNode::DefVar(var) => {
                // 初期値の中で終了した契約(定義した変数自身はまだ終了していない)
                let ends = self.walk_expr(&var.value, must_vars, fn_name, in_loop)?;
                if self.owns_contract(&var.ty) {
                    // 同名の契約変数が外側にあっても、既存の要素は書き換えず
                    // 新しい要素を積む。名前の検索は後ろから行うので内側が優先され、
                    // スコープを抜けるときの`truncate`で内側の要素だけが消える
                    must_vars
                        .borrow_mut()
                        .push(MustVar::new(var.name.clone(), &expr.span));
                } else {
                    let shadows = must_vars
                        .borrow()
                        .iter()
                        .any(|entry| entry.name == var.name);
                    if shadows {
                        // 契約のない変数が外側の契約変数を隠す場合
                        must_vars
                            .borrow_mut()
                            .push(MustVar::mask(var.name.clone(), &expr.span));
                    }
                }
                Ok(ends)
            }
            // 関数の呼び出し: 引数に渡された`must`の変数を消費済みにする
            ExprNode::CallFunc(call_info) => {
                let mut ends: Vec<String> = Vec::new();
                for arg in call_info.args.iter() {
                    if let Some(name) = Self::arg_var_name(arg) {
                        if Self::mark_used(name, must_vars) {
                            Self::push_ends(&mut ends, vec![name.clone()]);
                        }
                    }
                    let found = self.walk_expr(arg, must_vars, fn_name, in_loop)?;
                    Self::push_ends(&mut ends, found);
                }
                Ok(ends)
            }
            // `a.del()`: `must`のフィールドを持つ構造体の契約を、このスコープで終了する
            ExprNode::Member { scope, target } if Self::is_contract_end_call(target) => {
                let mut ends: Vec<String> = Vec::new();
                if let Some(name) = scope.last() {
                    if Self::mark_used(name, must_vars) {
                        Self::push_ends(&mut ends, vec![name.clone()]);
                    }
                }
                let found = self.walk_expr(target, must_vars, fn_name, in_loop)?;
                Self::push_ends(&mut ends, found);
                Ok(ends)
            }
            // 条件分岐(ループの外)は`branch.rs`の`walk_match`、反復処理は`loop_check.rs`で扱う
            ExprNode::Match { .. } => self.walk_match(expr, must_vars, fn_name, in_loop),
            ExprNode::Loop { .. } => self.walk_loop(expr, must_vars, fn_name),
            // それ以外は子の式を順に辿る(`try_for_each_child`を`scan_loop_expr`と共有する)
            _ => {
                let mut ends: Vec<String> = Vec::new();
                Self::try_for_each_child(expr, &mut |child| {
                    let found = self.walk_expr(child, must_vars, fn_name, in_loop)?;
                    Self::push_ends(&mut ends, found);
                    Ok(())
                })?;
                Ok(ends)
            }
        }
    }

    /// `&Box<Expr>`などを`&Expr`にそろえる(関数の引数での型強制を使う)
    pub(super) fn as_expr(expr: &Expr) -> &Expr {
        expr
    }

    /// 式の中で終了される(関数の引数として渡される)`must_vars`の変数名
    pub(super) fn ended_by(
        &mut self,
        expr: &Expr,
        must_vars: &MustVarsTy,
        fn_name: &str,
    ) -> Result<Vec<String>, err::ErrKind> {
        // `walk_expr`は、この式で新しく終了した変数だけを返す
        // (前の文で終了した変数が、あとの文すべてで終了したことにならない)
        // `ended_by`はループの本体の検査でだけ使うので、反復処理の中として辿る
        self.walk_expr(expr, must_vars, fn_name, true)
    }

    /// 式の子の式を、評価する順に`f`へ渡す
    ///
    /// `walk_expr`と`scan_loop_expr`が同じ一覧を使うための入口。
    /// 契約の追跡で特別な扱いが要るノード(`DefVar`/`CallFunc`/`Match`/`Loop`)は
    /// 含めず、呼び出し側で先に処理する。式ノードを追加したときは、
    /// 子の式を持つなら、ここに足す(足さないと追跡が漏れる)
    pub(super) fn try_for_each_child(
        expr: &Expr,
        f: &mut impl FnMut(&Expr) -> Result<(), err::ErrKind>,
    ) -> Result<(), err::ErrKind> {
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
            | ExprNode::AndCalcul(pair)
            | ExprNode::XorCalcul(pair)
            | ExprNode::OrCalcul(pair)
            | ExprNode::RShift(pair)
            | ExprNode::LShift(pair)
            | ExprNode::Surplus(pair)
            | ExprNode::CmpOr(pair)
            | ExprNode::CmpAnd(pair)
            | ExprNode::LessThen(pair)
            | ExprNode::GreaterThen(pair)
            | ExprNode::LessThenEq(pair)
            | ExprNode::GreaterThenEq(pair)
            | ExprNode::Equal(pair)
            | ExprNode::NotEq(pair) => {
                f(Self::as_expr(&pair.0))?;
                f(Self::as_expr(&pair.1))
            }
            ExprNode::Sizeof(..) => Ok(()),
            ExprNode::Cast((_, n)) | ExprNode::Sign((_, n)) => f(Self::as_expr(&n)),
            ExprNode::Ternary(n) => {
                f(Self::as_expr(&n.0))?;
                Self::check_tarnary_contract(&n.1, &n.2, &n.0.span)
            }
            ExprNode::Inc(n)
            | ExprNode::Inversion(n)
            | ExprNode::CmpNot(n)
            | ExprNode::LeftDec(n)
            | ExprNode::LeftInc(n)
            | ExprNode::Dec(n) => f(Self::as_expr(&n)),
            ExprNode::InitVariant { fields, .. } | ExprNode::InitStruct { fields, .. } => {
                fields.values().try_for_each(|val| f(Self::as_expr(val)))
            }
            ExprNode::Array(values) => values.iter().try_for_each(|val| f(Self::as_expr(val))),
            ExprNode::RefArray { dst, index, .. } => {
                f(Self::as_expr(dst))?;
                f(Self::as_expr(index))
            }
            // 検査しない
            ExprNode::EnumVariant { .. } => Ok(()),
            ExprNode::CallFunc(..)
            | ExprNode::Match { .. }
            | ExprNode::Loop { .. }
            | ExprNode::DefVar(..) => Ok(()),
            ExprNode::RangeNode(..) => Ok(()),
            ExprNode::Number(..) | ExprNode::Float(..) | ExprNode::Var(..) | ExprNode::Str(..) => {
                Ok(())
            }
        }
    }

    /// 引数として渡されて、契約を終了する変数の名前を返す
    ///
    /// `*var`のように包まれていても中身を辿る。
    /// `&var`(アドレスを取ってポインタとして渡す場合)は、契約した変数そのものを
    /// 渡す(譲渡する)わけではないので、契約は終了せず`None`を返す。
    pub(super) fn arg_var_name(arg: &Expr) -> Option<&String> {
        match &arg.node {
            ExprNode::Var(name) => Some(name),
            ExprNode::ConnectAddr(target) => Self::arg_var_name(&target),
            // ポインタで渡された契約は終了しない
            ExprNode::GetAddress(_) => None,
            _ => None,
        }
    }

    /// 新しく終了済みにしたら`true`(すでに終了済み・契約を持たない変数なら`false`)
    pub(super) fn mark_used(name: &str, must_vars: &MustVarsTy) -> bool {
        // 最も内側の同名の要素だけを見る。それが契約を持たない変数なら何もしない
        if let Some(var) = must_vars
            .borrow_mut()
            .iter_mut()
            .rev()
            .find(|var| var.name == name)
        {
            if var.tracked && !var.used {
                var.used = true;
                return true;
            }
        }
        false
    }

    /// 契約がすべて終了しているかを確認する関数
    pub(super) fn check_all_end_constract(
        &mut self,
        must_vars: &MustVarsTy,
        fn_name: &str,
        span: &Rc<Span>,
    ) -> Result<(), err::ErrKind> {
        // 一度も渡されていない変数、または条件分岐の一部でしか
        // 契約を終了していない変数はエラー
        for var in must_vars
            .borrow()
            .iter()
            .filter(|var| var.tracked && (!var.used || var.branch_mismatch))
        {
            (*self.err_pool.borrow_mut()).push_err_node(
                CompileErr::constract_must_not_used(&fn_name, &var.name, Rc::clone(&span))
                    .unwrap_err(),
            );
        }
        Ok(())
    }
}
