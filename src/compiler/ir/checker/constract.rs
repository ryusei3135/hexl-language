use super::*;
use crate::compiler::err::CompileErr;

// 条件分岐・反復処理・共通の処理は、それぞれ別のファイルに分けている
mod branch;
mod common;
mod loop_check;

use common::MustVar;

impl IR {
    /// 関数を呼び出すときの、引数1つ分の契約チェック
    ///
    /// - 引数の型が`of`なのに、渡された値が`must`でない -> エラー
    /// - 渡された値が`must`なのに、引数の型が`of`でない -> エラー
    /// - `must`の渡し先が呼び出し先と一致しない -> エラー
    pub(in crate::compiler::ir) fn check_constract_arg(
        &mut self,
        fn_name: &str,
        param: &node::ArgsNode,
        // 呼び出し元
        arg: &node::Expr,
    ) -> Result<(), err::ErrKind> {
        let arg_ty = self.expr_ty_node(arg);
        let arg_must = arg_ty.as_ref().and_then(node::TyNode::as_constract_must);
        let param_of = param.ty.as_constract_of();

        match (param_of, arg_must) {
            // `must=func`は、実際に値を渡す関数を指定する
            (Some(_of), Some(must)) => {
                // 渡す変数が契約しているのに渡す先の関数
                // の型が契約じゃない
                if must
                    .must_name()
                    .is_some_and(|name| name.as_str() != fn_name)
                {
                    CompileErr::constract_name_mismatch(
                        &fn_name,
                        &param.name,
                        node::ConstractTy::name_or_anon(must.must_name().map(|v| v.as_str())),
                        &fn_name,
                    )
                    .map_err(|err| err.with_span(Rc::clone(&self.current_span)))?
                }

                if let Some(var_name) = Self::arg_var_name(arg) {
                    self.var_tree
                        .finish_constract_var(var_name)
                        .map_err(|err| err.with_span(Rc::clone(&self.current_span)))
                } else {
                    Ok(())
                }
            }
            // `of`の引数には、`must`の値しか渡せない
            (Some(of), None) => CompileErr::constract_of_requires_must(
                &fn_name,
                &param.name,
                node::ConstractTy::name_or_anon(of.of_name().map(|v| v.as_str())),
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
                    node::ConstractTy::name_or_anon(must.must_name().map(|v| v.as_str())),
                )
                .map_err(|err| err.with_span(Rc::clone(&self.current_span)))
            }
            (None, None) => Ok(()),
        }
    }

    /// 呼び出し元と先の引数の型がそれぞれ`must`なら呼び出し先の
    /// 関数に契約を譲渡する
    /// ## 例
    /// ```
    /// a(): int must=c {}
    /// b(a: int must=c){}
    /// c(a: int of a){}
    /// ```
    #[inline(always)]
    fn move_constract_next_fn(
        &mut self,
        param: &node::ArgsNode,
        arg: &node::Expr,
        must: Option<&String>,
    ) -> Result<bool, err::ErrKind> {
        // 渡し先の引数が、同じ名前の`must`でなければ譲渡できない
        // (通常の引数や、別の名前の`must`の引数に渡したら契約が消える)
        let accepts_transfer = param
            .ty
            .as_constract_must()
            .is_some_and(|v| v.must_name() == must);
        if !accepts_transfer {
            return Ok(false);
        }

        if let Some(var_name) = Self::arg_var_name(arg) {
            self.var_tree
                .finish_constract_var(var_name)
                .map_err(|err| err.with_span(Rc::clone(&self.current_span)))?;

            Ok(true)
        } else {
            // 式から契約の移動元を特定できない。
            // 仕様に応じて明示的なコンパイルエラーにする。
            Ok(false)
        }
    }

    /// 式が変数を指している場合、その変数の型を返す
    /// (`&var`や`*var`のように、間接的に指している場合も辿る)
    fn expr_ty_node(&self, expr: &node::Expr) -> Option<node::TyNode> {
        match &expr.node {
            node::ExprNode::Var(name) => self.var_tree.get_ty_node(name),
            node::ExprNode::CallFunc(call) => self
                .func_tree
                .get(&call.name, None)
                .and_then(|func| func.ret_ty),
            node::ExprNode::GetAddress(target) | node::ExprNode::ConnectAddr(target) => {
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
    pub(crate) fn check_must_var_used(&self, func: &node::FuncDefine) {
        let mut must_vars: Vec<MustVar> = Vec::new();

        // 引数として受け取った`must`の値も、この関数の中で
        // 別の関数へ渡す義務がある
        func.params
            .iter()
            .filter(|param| param.ty.is_constract_must())
            .for_each(|param| {
                must_vars.push(MustVar::new(param.name.clone()));
            });

        Self::walk_group2_nodes(&func.body, &mut must_vars);

        // ループの中での契約の終了は、`ret`/`break`の前にしか置けない
        let mut scope: Vec<String> = must_vars.iter().map(|v| v.name.clone()).collect();
        scope.truncate(
            func.params
                .iter()
                .filter(|p| p.ty.is_constract_must())
                .count(),
        );
        Self::check_loop_contract_ends(&func.name, &func.body, &mut scope);

        // 一度も渡されていない変数、または条件分岐の一部でしか
        // 契約を終了していない変数はエラー
        for var in must_vars
            .iter()
            .filter(|var| (!var.used && !var.ended_on_exit) || var.branch_mismatch)
        {
            CompileErr::constract_must_not_used(&func.name, &var.name).unwrap();
        }
    }
}

// ===== テスト =====
#[cfg(test)]
mod constract_ir_test {
    use super::*;
    use crate::compiler::{lex, parse};

    /// 契約のテストで共通して使う関数の定義
    ///
    /// - `a`: 戻り値が`must=c`(受け取った側は`c`に渡す義務を持つ)
    /// - `c`/`d`: `of a`の引数を受け取る(`must=c`を受け取れるのは`c`だけ)
    /// - `e`: 契約のない通常の引数
    /// - `f`: `must=c`を受け取り、`c`へ譲渡する
    /// - `g`: `must=d`を受け取り、`d`へ譲渡する
    const DEFS: &str = "
a(): int must=c { ret 1 }
c(v: int of a) { }
d(v: int of a) { }
e(v: int) { }
f(v: int must=c) { c(v) }
g(v: int must=d) { d(v) }
";

    /// `main`の本体が`body`のソース全体を作る
    fn main_with(body: &str) -> String {
        format!("{DEFS}\nmain(): int {{\n{body}\nret 0\n}}")
    }

    /// ソースをIRにする。構文エラーはここで失敗させる(コンパイルエラーの
    /// テストが、構文の誤りで偶然通ってしまわないようにするため)
    ///
    /// ## Returns
    /// - `Ok`: IRの生成に成功した
    /// - `Err`: 契約などのコンパイルエラー(`Err`の返却とpanicの両方)
    ///
    /// `unwrap_or_report`はテスト中はpanicする(`std::process::exit`だと
    /// テスト全体が終了してしまうため。`ir/builder.rs`を参照)
    fn compile(src: &str) -> Result<(), String> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&src.to_string()).unwrap();
        let file_name = std::rc::Rc::new("<test>".to_string());
        let err_pool =
            std::rc::Rc::new(std::cell::RefCell::new(crate::err::err_pool::ErrPool::new()));
        let mut parser = parse::Parser::new(&file_name, err_pool);
        let nodes = parser
            .parser(&lexer.gen_tkns, &Vec::<String>::new())
            .unwrap()
            .to_vec();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut ir = IR::new();
            ir.builder(&nodes)
                .map(|_| ())
                .map_err(|err| format!("{:?}", err))
        }));
        match result {
            Ok(result) => result,
            Err(payload) => Err(payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".to_string())),
        }
    }

    #[track_caller]
    fn assert_ok(src: &str) {
        if let Err(err) = compile(src) {
            panic!("成功するはずが、エラーになりました: {err}\n{src}");
        }
    }

    #[track_caller]
    fn assert_err(src: &str) {
        if compile(src).is_ok() {
            panic!("エラーになるはずが、成功しました\n{src}");
        }
    }

    // ------------------------------------------------------------------
    // 1. `must`の変数を一度も譲渡しない -> コンパイルエラー
    // ------------------------------------------------------------------
    #[test]
    fn must_var_never_passed_is_error() {
        assert_ok(&main_with("x: int must=c = a()\nc(x)"));
        assert_err(&main_with("x: int must=c = a()"));
    }

    #[test]
    fn must_param_never_passed_is_error() {
        // 引数として受け取った`must`にも、渡す義務がある
        assert_ok("a(): int must=c { ret 1 }\nc(v: int of a) { }\nh(v: int must=c) { c(v) }");
        assert_err("a(): int must=c { ret 1 }\nc(v: int of a) { }\nh(v: int must=c) { }");
    }

    // ------------------------------------------------------------------
    // 2. 正しい`of`引数に譲渡する -> 成功
    // ------------------------------------------------------------------
    #[test]
    fn pass_to_matching_of_param_is_ok() {
        assert_ok(&main_with("x: int must=c = a()\nc(x)"));
    }

    #[test]
    fn transfer_to_must_param_with_same_name_is_ok() {
        // `f`は`must=c`を受け取って`c`へ渡すので、契約を肩代わりできる
        assert_ok(&main_with("x: int must=c = a()\nf(x)"));
    }

    #[test]
    fn transfer_to_must_param_with_other_name_is_error() {
        // `g`は`must=d`なので、`must=c`は譲渡できない
        assert_err(&main_with("x: int must=c = a()\ng(x)"));
    }

    // ------------------------------------------------------------------
    // 3. 異なる関数名の契約先に譲渡する -> コンパイルエラー
    // ------------------------------------------------------------------
    #[test]
    fn pass_to_other_function_is_error() {
        // `x`は`must=c`なので、`d`には渡せない
        assert_ok(&main_with("x: int must=c = a()\nc(x)"));
        assert_err(&main_with("x: int must=c = a()\nd(x)"));
    }

    // ------------------------------------------------------------------
    // 4. `must`の値を通常の引数に渡す -> コンパイルエラー
    // ------------------------------------------------------------------
    #[test]
    fn must_value_to_plain_param_is_error() {
        assert_ok(&main_with("x: int must=c = a()\nc(x)"));
        assert_err(&main_with("x: int must=c = a()\ne(x)"));
    }

    // ------------------------------------------------------------------
    // 5. `of`引数に通常の値を渡す -> コンパイルエラー
    // ------------------------------------------------------------------
    #[test]
    fn plain_value_to_of_param_is_error() {
        assert_ok(&main_with("x: int must=c = a()\nc(x)"));
        // 即値
        assert_err(&main_with("c(5)"));
        // 契約のない変数
        assert_err(&main_with("y: int = 1\nc(y)"));
    }

    // ------------------------------------------------------------------
    // 6. 契約を二度終了する -> コンパイルエラー
    // ------------------------------------------------------------------
    #[test]
    fn ending_contract_twice_is_error() {
        assert_ok(&main_with("x: int must=c = a()\nc(x)"));
        assert_err(&main_with("x: int must=c = a()\nc(x)\nc(x)"));
    }

    #[test]
    fn ending_contract_twice_across_functions_is_error() {
        // 一度`f`へ譲渡した契約は、もう`c`には渡せない
        assert_err(&main_with("x: int must=c = a()\nf(x)\nc(x)"));
    }

    #[test]
    fn ending_must_param_twice_is_error() {
        assert_err(
            "a(): int must=c { ret 1 }\nc(v: int of a) { }\nh(v: int must=c) { c(v)\nc(v) }",
        );
    }

    // ------------------------------------------------------------------
    // 7. 分岐の片方でしか契約を終了しない -> コンパイルエラー
    //    (現状の実装では検出できない。`#[ignore]`の理由を参照)
    // ------------------------------------------------------------------
    #[test]
    fn ending_contract_in_only_one_branch_is_error() {
        // 片方の分岐でしか終了しない(両方で終了する場合は、
        // 下の`each_branch_may_end_the_contract_once`を参照)
        assert_err(&main_with(
            "x: int must=c = a()\ncond 1 == 1 {\n c(x)\n} else {}",
        ));
        // もう片方の分岐だけで終了する場合
        assert_err(&main_with(
            "x: int must=c = a()\ncond 1 == 1 {\n} else {\n c(x)\n}",
        ));
        // `else`が無い場合
        assert_err(&main_with("x: int must=c = a()\ncond 1 == 1 {\n c(x)\n}"));
    }

    #[test]
    fn ending_contract_in_only_one_nested_branch_is_error() {
        assert_err(&main_with(
            "x: int must=c = a()\ncond 1 == 1 {\n cond 2 == 2 {\n c(x)\n } else {}\n} else {\n c(x)\n}",
        ));
    }

    #[test]
    fn ending_must_param_in_only_one_branch_is_error() {
        assert_err(
            "a(): int must=c { ret 1 }\nc(v: int of a) { }\nh(v: int must=c) { cond 1 == 1 {\n c(v)\n} else {} }",
        );
    }

    #[test]
    #[ignore = "未実装: 各分岐で一度ずつ終了しているのに、2つ目の分岐が`二重終了`として弾かれる"]
    fn each_branch_may_end_the_contract_once() {
        assert_ok(&main_with(
            "x: int must=c = a()\ncond 1 == 1 {\n c(x)\n} else {\n c(x)\n}",
        ));
    }

    // ------------------------------------------------------------------
    // 8. ループ内で契約を繰り返し終了する -> 二重終了を許可しない
    // ------------------------------------------------------------------
    #[test]
    fn ending_outer_contract_inside_loop_is_error() {
        assert_err(&main_with("x: int must=c = a()\nloop {\n c(x)\n}"));
    }

    #[test]
    fn ending_outer_contract_inside_loop_is_ok_only_before_ret_or_break() {
        assert_ok(&main_with("x: int must=c = a()\nloop {\n c(x)\n break\n}"));
        assert_ok(&main_with("x: int must=c = a()\nloop {\n c(x)\n ret 0\n}"));
        // `continue`では次の周で二重に終了してしまう
        assert_err(&main_with(
            "x: int must=c = a()\nloop {\n c(x)\n continue\n}",
        ));
        // 終了と`break`の間に別の文を挟めない
        assert_err(&main_with(
            "x: int must=c = a()\nloop {\n c(x)\n y: int = 1\n break\n}",
        ));
        // 分岐の中でも、その分岐の中の`ret`/`break`の前でなければならない
        assert_ok(&main_with(
            "x: int must=c = a()\nloop {\n cond 1 == 1 {\n c(x)\n break\n } else {}\n break\n}",
        ));
        assert_err(&main_with(
            "x: int must=c = a()\nloop {\n cond 1 == 1 {\n c(x)\n } else {}\n break\n}",
        ));
    }

    #[test]
    fn contract_declared_and_ended_inside_loop_is_ok() {
        // ループの中で毎周作って毎周終了するなら、二重終了ではない
        assert_ok(&main_with("loop {\n x: int must=c = a()\n c(x)\n break\n}"));
    }

    // ------------------------------------------------------------------
    // 9. 同名の変数を別スコープで宣言する -> それぞれ独立して追跡する
    // ------------------------------------------------------------------
    #[test]
    fn same_name_in_sibling_scopes_is_tracked_independently() {
        let both_ended = "cond 1 == 1 {
 x: int must=c = a()
 c(x)
} else {
 x: int must=c = a()
 c(x)
}";
        assert_ok(&main_with(both_ended));

        // 片方のスコープの`x`だけ終了していなければ、そのスコープの`x`がエラーになる
        let second_not_ended = "cond 1 == 1 {
 x: int must=c = a()
 c(x)
} else {
 x: int must=c = a()
}";
        assert_err(&main_with(second_not_ended));

        let first_not_ended = "cond 1 == 1 {
 x: int must=c = a()
} else {
 x: int must=c = a()
 c(x)
}";
        assert_err(&main_with(first_not_ended));
    }

    #[test]
    fn same_name_must_and_plain_var_in_sibling_scopes() {
        // 片方のスコープの`x`が契約のない変数でも、互いに影響しない
        assert_ok(&main_with(
            "cond 1 == 1 {\n x: int must=c = a()\n c(x)\n} else {\n x: int = 1\n}",
        ));
    }

    // ------------------------------------------------------------------
    // 10. 契約付き関数呼び出しの戻り値を渡す -> 仕様に応じた正しい検査
    //     (変数を介さないので終了する対象の変数が無く、渡し先だけを検査する)
    // ------------------------------------------------------------------
    #[test]
    fn call_result_passed_to_matching_of_param_is_ok() {
        assert_ok(&main_with("c(a())"));
    }

    #[test]
    fn call_result_passed_to_other_function_is_error() {
        assert_err(&main_with("d(a())"));
    }

    #[test]
    fn call_result_passed_to_plain_param_is_error() {
        assert_err(&main_with("e(a())"));
    }

    #[test]
    fn plain_call_result_passed_to_of_param_is_error() {
        // 契約のない関数の戻り値は`of`に渡せない
        assert_err(&format!(
            "{DEFS}\nb(): int {{ ret 1 }}\nmain(): int {{\n c(b())\n ret 0\n}}"
        ));
    }

    // ------------------------------------------------------------------
    // 11. return / break / continue でスコープを抜ける -> 契約の義務を正しく処理する
    // ------------------------------------------------------------------
    #[test]
    fn break_with_unfinished_contract_declared_in_loop_is_error() {
        // ループの中で作った契約を終了せずに`break`で抜ける
        assert_err(&main_with("loop {\n x: int must=c = a()\n break\n}"));
        // 終了してから抜けるなら成功
        assert_ok(&main_with("loop {\n x: int must=c = a()\n c(x)\n break\n}"));
    }

    #[test]
    fn continue_with_unfinished_contract_is_error() {
        // 終了しないまま次の周へ進む(その変数はどこでも終了されない)
        assert_err(&main_with("loop {\n x: int must=c = a()\n continue\n}"));
    }

    #[test]
    fn return_with_unfinished_contract_is_error() {
        // `ret`の前にも後にも終了していない
        assert_err(&format!(
            "{DEFS}\nmain(): int {{\n x: int must=c = a()\n ret 0\n}}"
        ));
        // `ret`の前に終了していれば成功
        assert_ok(&format!(
            "{DEFS}\nmain(): int {{\n x: int must=c = a()\n c(x)\n ret 0\n}}"
        ));
    }

    #[test]
    #[ignore = "未実装: `ret`/`continue`で抜ける経路ごとの契約の検査はなく、\
                関数のどこかで譲渡されていれば通ってしまう"]
    fn early_return_before_ending_contract_is_error() {
        assert_err(&main_with(
            "x: int must=c = a()\ncond 1 == 1 {\n ret 1\n}\nc(x)",
        ));
    }

    #[test]
    #[ignore = "未実装: `continue`で次の周へ進む経路の契約の検査はない"]
    fn continue_before_ending_contract_is_error() {
        assert_err(&main_with(
            "loop {\n x: int must=c = a()\n cond 1 == 1 {\n continue\n } else {}\n c(x)\n break\n}",
        ));
    }
}
