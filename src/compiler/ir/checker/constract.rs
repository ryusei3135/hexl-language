use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::compiler::ast::{ArgsNode, ConstractTy, Expr, ExprNode, FuncDefine, TyNode};
use crate::compiler::err::CompileErr;
use crate::compiler::optimization::x64::gen_ir::tracking_contract_len::{
    contract_mem_len, var_len_from_tree,
};

// 条件分岐・反復処理・共通の処理は、それぞれ別のファイルに分けている
mod branch;
mod common;
#[cfg(test)]
mod contract_test;
mod embe_expr;
mod loop_check;

use common::{MustVar, MustVarsTy};

/// 契約の検査から見た、式の型
enum ExprTy {
    /// 型が確定した
    Known(TyNode),
    /// 契約を持ち得ない、通常の値
    Plain,
    /// 契約を持ち得るが、型を確定できなかった(検査不能)
    Unresolved,
}

impl IR {
    /// 関数を呼び出すときの、引数1つ分の契約チェック
    ///
    /// - 引数の型が`of`なのに、渡された値が`must`でない -> エラー
    /// - 渡された値が`must`なのに、引数の型が`of`でない -> エラー
    /// - `must`の渡し先が呼び出し先と一致しない -> エラー
    /// - 引数の型が`RefContract`なら、契約を移さずに借りる(`check_ref_contract_arg`)
    /// - 借りている契約(`RefContract`の値)は、`of`や`must`の引数へ渡せない -> エラー
    pub(in crate::compiler::ir) fn check_constract_arg(
        &mut self,
        fn_name: &str,
        fn_module: Option<&str>,
        param: &ArgsNode,
        // 呼び出し元
        arg: &Expr,
    ) -> Result<(), err::ErrKind> {
        let arg_ty = self.expr_ty_node(arg);

        // 契約を借りる引数: 契約は移動しない
        if param.ty.is_ref_contract() {
            return self.check_ref_contract_arg(fn_name, param, arg, &arg_ty);
        }
        // 借りているだけの契約は、呼び出し先でも終了できない
        // (終了する`of`の引数にも、契約を引き取る`must`の引数にも渡せない)
        if matches!(&arg_ty, ExprTy::Known(ty) if ty.is_ref_contract())
            && (param.ty.is_contract_of() || param.ty.is_contract_must())
        {
            let var_name = Self::contract_var_name(arg).unwrap_or(&param.name);
            return def_tree::VarTree::ref_contract_cannot_end(var_name, Rc::clone(&arg.span))
                .map_err(|err| err.with_span(Rc::clone(&self.current_span)));
        }

        // 型を確定できなかった式は、通常の値として扱わない。
        // `of`の引数には「`must`の値だ」と確認できた式しか渡せないので、
        // 確認できない式は`must`ではない値と同じエラーにする
        let arg_must = match &arg_ty {
            ExprTy::Known(ty) => ty.as_contract_must(),
            ExprTy::Plain | ExprTy::Unresolved => None,
        };
        // 通常の`self: Self*`は受け手を借りるだけで、所有契約を移動しない
        if arg_must.is_some() && Self::is_borrowed_method_receiver(fn_module, param) {
            if let Some(var_name) = Self::contract_var_name(arg) {
                self.var_tree
                    .ensure_contract_alive(var_name, Rc::clone(&arg.span))
                    .map_err(|err| err.with_span(Rc::clone(&self.current_span)))?;
            }
            return Ok(());
        }
        let param_of = param.ty.as_contract_of();

        match (param_of, arg_must) {
            // `must=func`は、実際に値を渡す関数を指定する
            (Some(_of), Some(must)) => {
                // 渡す変数が契約しているのに渡す先の関数
                // の型が契約じゃない
                let expected_fn = must.must_name();
                // メソッドの契約名は所属型で修飾される。通常の関数は従来どおり関数名で照合する
                let actual_fn = expected_fn
                    .filter(|name| name.contains("::"))
                    .and_then(|_| fn_module.map(|module| format!("{module}::{fn_name}")));
                let matches_fn = expected_fn.map_or(true, |expected| {
                    if expected.contains("::") {
                        actual_fn.as_deref() == Some(expected.as_str())
                    } else {
                        expected.as_str() == fn_name
                    }
                });
                if !matches_fn {
                    CompileErr::constract_name_mismatch(
                        &fn_name,
                        &param.name,
                        ConstractTy::name_or_anon(must.must_name().map(|v| v.as_str())),
                        &fn_name,
                        Rc::clone(&arg.span),
                    )
                    .map_err(|err| err.with_span(Rc::clone(&self.current_span)))?
                }

                if let Some(var_name) = Self::arg_var_name(arg) {
                    self.var_tree
                        .finish_constract_var(var_name, Rc::clone(&arg.span))
                        .map_err(|err| err.with_span(Rc::clone(&self.current_span)))
                } else {
                    Ok(())
                }
            }
            // `of`の引数には、`must`の値しか渡せない
            (Some(of), None) => CompileErr::constract_of_requires_must(
                &fn_name,
                &param.name,
                ConstractTy::name_or_anon(of.of_name().map(|v| v.as_str())),
                Rc::clone(&arg.span),
            )
            .map_err(|err| err.with_span(Rc::clone(&self.current_span))),
            // `must`の値は、`of`の引数にしか渡せない
            (None, Some(must)) => {
                // 引数が契約を譲渡するならOk
                if self.move_constract_next_fn(&param, &arg, must.must_name())? {
                    return Ok(());
                };
                //
                CompileErr::constract_must_requires_of(
                    &fn_name,
                    &param.name,
                    ConstractTy::name_or_anon(must.must_name().map(|v| v.as_str())),
                    Rc::clone(&arg.span),
                )
                .map_err(|err| err.with_span(Rc::clone(&self.current_span)))
            }
            (None, None) => Ok(()),
        }
    }

    fn is_borrowed_method_receiver(fn_module: Option<&str>, param: &ArgsNode) -> bool {
        let Some(owner) = fn_module else {
            return false;
        };
        param.name == "self"
            && matches!(
                &param.ty,
                TyNode::Pointer { ty_name, .. }
                    if matches!(&**ty_name, TyNode::Ty(name) if name == owner)
            )
    }

    /// 契約を借りる(`RefContract`)引数1つ分のチェック
    ///
    /// 契約は移動しないので、渡した変数の契約は終了済みにならない
    /// (呼び出し元が、これまでどおり自分で終了する)
    ///
    /// - 渡す値が契約を持っていない(`must`でも`RefContract`でもない) -> エラー
    /// - 契約名が、引数の型の契約名と一致しない -> エラー
    /// - 渡す変数の契約がすでに終了している -> エラー
    fn check_ref_contract_arg(
        &mut self,
        fn_name: &str,
        param: &ArgsNode,
        arg: &Expr,
        arg_ty: &ExprTy,
    ) -> Result<(), err::ErrKind> {
        // 引数の型の契約名(`int ref must=c`の`c`)。指定が無ければ、どの契約でも借りられる
        let expected = param
            .ty
            .as_ref_contract()
            .and_then(|inner| inner.as_contract_must())
            .and_then(|must| must.must_name());

        // 渡す値の契約名。型を確定できなかった式は、契約を持つとは認めない
        // (`of`の引数と同じ扱い)
        let held = match arg_ty {
            ExprTy::Known(ty) => ty.held_contract_name(),
            ExprTy::Plain | ExprTy::Unresolved => None,
        };
        let Some(held) = held else {
            if param.unsafe_contract_ptr_arithmetic {
                if let Some(base) = self.contract_ptr_arithmetic_base(arg, expected) {
                    if let Some(var_name) = Self::contract_var_name(base) {
                        self.var_tree
                            .ensure_contract_alive(var_name, Rc::clone(&arg.span))
                            .map_err(|err| err.with_span(Rc::clone(&self.current_span)))?;
                    }
                    return Ok(());
                }
            }
            return CompileErr::constract_of_requires_must(
                &fn_name,
                &param.name,
                ConstractTy::name_or_anon(expected.map(|v| v.as_str())),
                Rc::clone(&arg.span),
            )
            .map_err(|err| err.with_span(Rc::clone(&self.current_span)));
        };

        if let (Some(held_name), Some(expected_name)) = (held, expected) {
            if held_name != expected_name {
                return CompileErr::constract_name_mismatch(
                    &fn_name,
                    &param.name,
                    ConstractTy::name_or_anon(Some(held_name.as_str())),
                    &expected_name.as_str(),
                    Rc::clone(&arg.span),
                )
                .map_err(|err| err.with_span(Rc::clone(&self.current_span)));
            }
        }

        // すでに終了した契約は借りられない。終了は記録しない(移動ではないので)
        if let Some(var_name) = Self::contract_var_name(arg) {
            self.var_tree
                .ensure_contract_alive(var_name, Rc::clone(&arg.span))
                .map_err(|err| err.with_span(Rc::clone(&self.current_span)))?;
        }
        Ok(())
    }

    fn contract_ptr_arithmetic_base<'a>(
        &self,
        arg: &'a Expr,
        expected: Option<&String>,
    ) -> Option<&'a Expr> {
        if let ExprTy::Known(ty) = self.expr_ty_node(arg) {
            if let Some(contract) = ty.as_contract_must() {
                let name_matches = expected.map_or(true, |expected| {
                    contract.must_name().map(String::as_str) == Some(expected)
                });
                if name_matches && matches!(contract.unwrap_ty(), TyNode::Pointer { .. }) {
                    return Some(arg);
                }
            }
        }

        match &arg.node {
            ExprNode::Add((left, right)) | ExprNode::Sub((left, right)) => self
                .contract_ptr_arithmetic_base(left, expected)
                .or_else(|| self.contract_ptr_arithmetic_base(right, expected)),
            _ => None,
        }
    }

    /// 契約を持つ値を渡す変数の名前(`&x` / `*x`のように包まれていても中身を辿る)
    ///
    /// `arg_var_name`と違い、`&x`も辿る(契約は移さず、変数の状態を調べるためだけに使う)
    fn contract_var_name(arg: &Expr) -> Option<&String> {
        match &arg.node {
            ExprNode::Var(name) => Some(name),
            ExprNode::ConnectAddr(target) | ExprNode::GetAddress(target) => {
                Self::contract_var_name(target)
            }
            _ => None,
        }
    }

    /// 呼び出し元と先の引数の型がそれぞれ`must`なら呼び出し先の
    /// 関数に契約を譲渡する
    /// ## 例
    /// ```txt
    /// a(): int must=c {}
    /// b(a: int must=c){}
    /// c(a: int of a){}
    /// ```
    #[inline(always)]
    fn move_constract_next_fn(
        &mut self,
        param: &ArgsNode,
        arg: &Expr,
        must: Option<&String>,
    ) -> Result<bool, err::ErrKind> {
        // 渡し先の引数が、同じ名前の`must`でなければ譲渡できない
        // (通常の引数や、別の名前の`must`の引数に渡したら契約が消える)
        let accepts_transfer = param
            .ty
            .as_contract_must()
            .is_some_and(|v| v.must_name() == must);
        if !accepts_transfer {
            return Ok(false);
        }

        if let Some(var_name) = Self::arg_var_name(arg) {
            self.var_tree
                .finish_constract_var(var_name, Rc::clone(&arg.span))
                .map_err(|err| err.with_span(Rc::clone(&self.current_span)))?;

            Ok(true)
        } else if Self::is_ptr_member_arg(arg) {
            // `self->val` / `[ptr].val`のような、構造体のメンバーの`must`。
            // メンバーの契約は変数単位では追跡せず、構造体を初期化した
            // スコープで終了を確認する。`must=..`の引数へ渡したので、
            // ここでは契約のチェックを外す(譲渡済みとして扱う)
            Ok(true)
        } else {
            // 式から契約の移動元を特定できない。
            // 仕様に応じて明示的なコンパイルエラーにする。
            Ok(false)
        }
    }

    /// 引数が、ポインタ経由の構造体のメンバー(`self->val` / `[ptr].val`)か
    /// (`*`で包まれていても中身を辿る)
    fn is_ptr_member_arg(arg: &Expr) -> bool {
        match &arg.node {
            ExprNode::ConnectAddr(target) => Self::is_ptr_member_arg(target),
            ExprNode::PtrMember { target, .. } => matches!(target.node, ExprNode::Var(_)),
            _ => false,
        }
    }

    /// 処理中の関数が、構造体の初期化メゾット(`Struct::new`)か
    pub(in crate::compiler::ir) fn in_constructor(&self) -> bool {
        self.define_meta_data
            .last()
            .is_some_and(|f| f.name == "new" && f.module().is_some())
    }

    /// ポインタ変数`ptr_name`が指す構造体の、メンバー`member`の宣言上の型を返す
    /// (`self->name`/`[ptr].name`の`name`の型)。
    /// 構造体やメンバーが見つからない場合は`None`
    pub(in crate::compiler::ir) fn ptr_member_field_ty(
        &self,
        ptr_name: &str,
        member: &str,
    ) -> Option<TyNode> {
        let struct_name = self.var_tree.get_ty_name(ptr_name).ok()?;
        self.struct_tree
            .get(&struct_name)?
            .fields
            .iter()
            .find(|field| field.name == member)
            .map(|field| field.ty.clone())
    }

    /// `ptr_member_field_ty`の、契約の検査用(`ExprTy`)版。見つからなければ`Unresolved`
    fn ptr_member_ty(&self, ptr_name: &str, member: &str) -> ExprTy {
        self.ptr_member_field_ty(ptr_name, member)
            .map_or(ExprTy::Unresolved, ExprTy::Known)
    }

    /// 契約の検査のために、式の型を調べる
    /// (`&var`や`*var`のように、間接的に指している場合も辿る)
    ///
    /// 型を確定できない式を`Plain`(契約のない通常の値)として返さないこと。
    /// 契約を持つ値を表せる式で型を解決できなかった場合は`Unresolved`を返す
    fn expr_ty_node(&self, expr: &Expr) -> ExprTy {
        match &expr.node {
            ExprNode::Var(name) => self
                .var_tree
                .get_ty_node(name)
                .map_or(ExprTy::Unresolved, ExprTy::Known),
            ExprNode::CallFunc(call) => match self.func_tree.get(&call.name, None) {
                Some(func) => func.ret_ty.map_or(ExprTy::Plain, ExprTy::Known),
                // 呼び出し先が見つからない場合のエラーは、呼び出しのIR生成が報告する
                None => ExprTy::Plain,
            },
            ExprNode::Scope { scope, target } => match &target.node {
                ExprNode::CallFunc(call) => scope
                    .last()
                    .and_then(|module| self.method_ret_ty(call, module))
                    .map_or(ExprTy::Plain, ExprTy::Known),
                _ => ExprTy::Unresolved,
            },
            ExprNode::Member { scope, target } => match &target.node {
                ExprNode::CallFunc(call) => scope
                    .last()
                    .and_then(|receiver| self.var_tree.get_ty_name(receiver).ok())
                    .and_then(|module| self.method_ret_ty(call, &module))
                    .map_or(ExprTy::Plain, ExprTy::Known),
                _ => ExprTy::Unresolved,
            },
            ExprNode::PtrMember { name, target, .. } => match &target.node {
                ExprNode::CallFunc(call) => self
                    .var_tree
                    .get_ty_name(name)
                    .ok()
                    .and_then(|module| self.method_ret_ty(call, &module))
                    .map_or(ExprTy::Plain, ExprTy::Known),
                ExprNode::Var(member) => self.ptr_member_ty(name, member),
                _ => ExprTy::Unresolved,
            },
            ExprNode::GetAddress(target) | ExprNode::ConnectAddr(target) => {
                self.expr_ty_node(target)
            }
            // 代入式の値の型は、代入先の型
            ExprNode::Assign(assign) => self.expr_ty_node(&assign.dst),
            // 契約を持つ値を取り出せる式だが、ここでは型を解決できない
            // (メンバー・配列の要素・分岐やループの値・変数の定義)
            ExprNode::Member { .. }
            | ExprNode::RefArray { .. }
            | ExprNode::VariantBind(_)
            | ExprNode::DefVar(_)
            | ExprNode::Match { .. }
            | ExprNode::Loop { .. } => ExprTy::Unresolved,
            // 即値・算術・比較・構造体や配列の初期化など、新しく作られる値。
            // 契約(`must`)は関数の戻り値からしか生まれないので、通常の値
            _ => ExprTy::Plain,
        }
    }

    fn method_ret_ty(&self, call: &crate::compiler::ast::CallInfo, module: &str) -> Option<TyNode> {
        self.func_tree
            .get_with_temp(&call.name, Some(module), &call.temp_ty)
            .and_then(|func| func.ret_ty)
    }

    /// `must`の引数が、戻り値の`must`へ契約を引き継ぐか
    ///
    /// 引数の型と戻り値の型が同じ名前の`must`(`realloc(ptr: T* must=free): T* must=free`)なら、
    /// 呼び出し元が渡した契約は、戻り値を受け取った側へ引き継がれる。
    /// この引数は、関数の中で別の関数へ渡さなくてよい
    /// (途中で`free`などへ渡して終了しても構わない)
    fn hands_over_to_ret(func: &FuncDefine, param: &ArgsNode) -> bool {
        let ret_ty = func.contract.as_ref().unwrap_or(&func.ret_ty);
        let Some(ret_must) = ret_ty.as_contract_must() else {
            return false;
        };
        ret_must.must_name().is_some()
            && param
                .ty
                .as_contract_must()
                .is_some_and(|m| m.must_name() == ret_must.must_name())
    }

    /// `must`の契約を持つ変数が、関数の中で必ず一度は
    /// 関数の引数として渡されているかを確認する
    ///
    /// 渡した先の引数が本当に`of`かどうか(名前が一致するか)は、
    /// IRを生成する時点で`check_constract_arg`が確認するので、
    /// ここでは「そもそも関数に渡されているか」だけを見る
    pub(crate) fn check_must_var_used(&mut self, func: &FuncDefine) -> Result<(), err::ErrKind> {
        let must_vars: MustVarsTy = Rc::new(RefCell::new(Vec::new()));

        // 引数として受け取った`must`の値も、この関数の中で
        // 別の関数へ渡す義務がある
        // (戻り値の`must`へ引き継ぐ引数は、その義務を戻り値が負うので除く)
        func.params
            .iter()
            .filter(|param| param.ty.is_contract_must() && !Self::hands_over_to_ret(func, param))
            .for_each(|param| {
                must_vars
                    .borrow_mut()
                    .push(MustVar::new(param.name.clone(), &func.span));
            });
        // 契約を借りた引数(`RefContract`)は、終了する義務が無い代わりに、
        // この関数の中で終了することもできない(`del`などをエラーにするための印)
        func.params
            .iter()
            .filter(|param| param.ty.is_ref_contract())
            .for_each(|param| {
                must_vars
                    .borrow_mut()
                    .push(MustVar::borrowed(param.name.clone(), &func.span));
            });

        self.walk_group2_nodes(&func.body, &must_vars, &func.name, false)?;

        // ループの中での契約の終了は、`ret`/`break`の前にしか置けない。
        // `scan_loop_expr`は`DefVar`を見つけるたびに`scope`へ変数を積むので、
        // `must_vars`とは別の一覧(`must`の引数だけを入れたもの)を渡す。
        // 同じ一覧を渡すと、`walk_group2_nodes`が積んだ変数と同名の「未終了の変数」が
        // もう一つ増え、`check_all_end_constract`が誤ってエラーにする
        let loop_scope: MustVarsTy = Rc::new(RefCell::new(
            func.params
                .iter()
                .filter(|param| {
                    param.ty.is_contract_must() && !Self::hands_over_to_ret(func, param)
                })
                .map(|param| MustVar::new(param.name.clone(), &func.span))
                .collect(),
        ));
        self.check_loop_contract_ends(&func.name, &func.body, &loop_scope)?;

        // 一度も渡されていない変数、または条件分岐の一部でしか
        // 契約を終了していない変数はエラー
        // 構造体のメゾット(第一引数が`Self`のものを含む)は、契約を持つ値を
        // フィールドへ入れたり、`self`経由で扱ったりするので、関数が終わるときに
        // 契約が終了していなくてもエラーにしない
        // (契約の終了は、構造体を初期化したスコープで確認する)
        if Self::is_struct_method(func) {
            return Ok(());
        }
        self.check_all_end_constract(&must_vars, &func.name, &func.span)
    }

    /// 構造体のメゾット、または第一引数が`Self`(`Self*`)の関数か
    fn is_struct_method(func: &FuncDefine) -> bool {
        func.module.is_some()
            || func
                .params
                .first()
                .is_some_and(|param| Self::ty_contains_self(&param.ty))
    }
}
