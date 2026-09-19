use super::*;
use crate::err::CompileErr;

/// 関数の中で追跡している、`must`の契約を持つ変数
#[derive(Clone, Debug)]
struct MustVar {
    name: String,
    /// 一度でも関数の引数として渡されたら`true`
    used: bool,
}

impl IR {
    /// 関数を呼び出すときの、引数1つ分の契約チェック
    ///
    /// - 引数の型が`of`なのに、渡された値が`must`でない -> エラー
    /// - 渡された値が`must`なのに、引数の型が`of`でない -> エラー
    /// - `must`の渡し先が呼び出し先と一致しない -> エラー
    pub(crate) fn check_constract_arg(
        &self,
        fn_name: &String,
        param: &node::ArgsNode,
        arg: &node::Expr,
    ) {
        let arg_ty = self.expr_ty_node(arg);
        let arg_must = arg_ty
            .as_ref()
            .and_then(node::TyNode::as_constract_must);
        let param_of = param.ty.as_constract_of();

        match (param_of, arg_must) {
            // `must=func`は、実際に値を渡す関数を指定する
            (Some(_of), Some(must)) => {
                if must.must_name().is_some_and(|name| name != fn_name) {
                    CompileErr::constract_name_mismatch(
                        fn_name,
                        &param.name,
                        node::ConstractTy::name_or_anon(must.must_name()),
                        fn_name,
                    )
                    .map_err(|err| err.with_span(self.current_span))
                    .unwrap();
                }
            }
            // `of`の引数には、`must`の値しか渡せない
            (Some(of), None) => {
                CompileErr::constract_of_requires_must(
                    fn_name,
                    &param.name,
                    node::ConstractTy::name_or_anon(of.of_name()),
                )
                .map_err(|err| err.with_span(self.current_span))
                .unwrap();
            }
            // `must`の値は、`of`の引数にしか渡せない
            (None, Some(must)) => {
                CompileErr::constract_must_requires_of(
                    fn_name,
                    &param.name,
                    node::ConstractTy::name_or_anon(must.must_name()),
                )
                .map_err(|err| err.with_span(self.current_span))
                .unwrap();
            }
            (None, None) => {}
        }
    }

    /// 式が変数を指している場合、その変数の型を返す
    /// (`&var`や`*var`のように、間接的に指している場合も辿る)
    fn expr_ty_node(
        &self, 
        expr: &node::Expr
    ) -> Option<node::TyNode> {
        match expr {
            node::Expr::Var(name) => {
                self.var_tree.get_ty_node(name)
            }
            node::Expr::CallFunc(call) => self
                .func_tree
                .get(&call.name, None)
                .and_then(|func| func.ret_ty),
            node::Expr::GetAddress(target)
            | node::Expr::ConnectAddr(target) => {
                self.expr_ty_node(target)
            }
            _ => None,
        }
    }

    /// `must`の契約を持つ変数が、関数の中で必ず一度は
    /// 関数の引数として渡されているかを確認する
    ///
    /// 渡した先の引数が本当に`of`かどうか(名前が一致するか)は、
    /// IRを生成する時点で`check_constract_arg`が確認するので、
    /// ここでは「そもそも関数に渡されているか」だけを見る
    pub(crate) fn check_must_var_used(
        &self, 
        func: &node::FuncDefine
    ) {
        let mut must_vars: Vec<MustVar> = Vec::new();

        // 引数として受け取った`must`の値も、この関数の中で
        // 別の関数へ渡す義務がある
        for param in func.params.iter() {
            if param.ty.is_constract_must() {
                must_vars.push(MustVar {
                    name: param.name.clone(),
                    used: false,
                });
            }
        }

        Self::walk_group2_nodes(&func.body, &mut must_vars);

        for var in must_vars.iter() {
            if !var.used {
                CompileErr::constract_must_not_used(
                    &func.name, 
                    &var.name
                )
                .unwrap();
            }
        }
    }

    fn walk_group2_nodes(
        nodes: &Body,
        must_vars: &mut Vec<MustVar>,
    ) {
        for n in nodes.iter() {
            match n.get_node() {
                node::Group2Node::Expr(ref expr) => {
                    Self::walk_expr(&expr, must_vars);
                }
                node::Group2Node::Stmt(
                    node::StmtNode::Return(ref expr)
                ) => {
                    Self::walk_expr(&expr, must_vars);
                }
                _ => {}
            }
        }
    }

    fn walk_expr(
        expr: &node::Expr, 
        must_vars: &mut Vec<MustVar>
    ) {
        match expr {
            // 変数の定義: `must`の型ならここから追跡を始める
            node::Expr::DefVar(var) => {
                Self::walk_expr(&var.value, must_vars);
                if var.ty.is_constract_must() {
                    must_vars.push(MustVar {
                        name: var.name.clone(),
                        used: false,
                    });
                }
            }
            // 関数の呼び出し: 引数に渡された`must`の変数を消費済みにする
            node::Expr::CallFunc(call_info) => {
                for arg in call_info.args.iter() {
                    if let Some(name) = Self::arg_var_name(arg) {
                        Self::mark_used(name, must_vars);
                    }
                    Self::walk_expr(arg, must_vars);
                }
            }
            node::Expr::Scope { target, .. }
            | node::Expr::Member { target, .. } => {
                Self::walk_expr(target, must_vars);
            }
            node::Expr::Assign(assign) => {
                Self::walk_expr(&assign.dst, must_vars);
                Self::walk_expr(&assign.value, must_vars);
            }
            node::Expr::GetAddress(target)
            | node::Expr::ConnectAddr(target) => {
                Self::walk_expr(target, must_vars);
            }
            node::Expr::Add(pair)
            | node::Expr::Sub(pair)
            | node::Expr::Mul(pair)
            | node::Expr::Div(pair)
            | node::Expr::Surplus(pair)
            | node::Expr::LessThen(pair)
            | node::Expr::GreaterThen(pair)
            | node::Expr::Equal(pair)
            | node::Expr::NotEq(pair) => {
                Self::walk_expr(&pair.0, must_vars);
                Self::walk_expr(&pair.1, must_vars);
            }
            node::Expr::Match {
                pattern,
                arms,
                arm_else,
            } => {
                if let Some(pattern) = pattern {
                    Self::walk_expr(pattern, must_vars);
                }
                for arm in arms.iter() {
                    Self::walk_expr(&arm.pattern, must_vars);
                    Self::walk_group2_nodes(&arm.body, must_vars);
                }
                if let Some(body) = arm_else {
                    Self::walk_group2_nodes(body, must_vars);
                }
            }
            node::Expr::Loop { pattern, body } => {
                if let Some(pattern) = pattern {
                    Self::walk_expr(pattern, must_vars);
                }
                Self::walk_group2_nodes(body, must_vars);
            }
            node::Expr::InitStruct { fields, .. } => {
                for value in fields.values() {
                    Self::walk_expr(value, must_vars);
                }
            }
            node::Expr::Array(values) => {
                for value in values.iter() {
                    Self::walk_expr(value, must_vars);
                }
            }
            node::Expr::RefArray { 
                dst, 
                index, .. 
            } => {
                Self::walk_expr(dst, must_vars);
                Self::walk_expr(index, must_vars);
            }
            _ => {}
        }
    }

    /// 引数の式が指している変数の名前を返す
    /// (`&var`や`*var`のように包まれていても中身を辿る)
    fn arg_var_name(
        arg: &node::Expr
    ) -> Option<&String> {
        match arg {
            node::Expr::Var(name) => Some(name),
            node::Expr::GetAddress(target)
            | node::Expr::ConnectAddr(target) => {
                Self::arg_var_name(target)
            }
            _ => None,
        }
    }

    fn mark_used(
        name: &String, 
        must_vars: &mut Vec<MustVar>
    ) {
        if let Some(var) = must_vars
            .iter_mut()
            .find(|var| &var.name == name) 
        {
            var.used = true;
        }
    }
}
