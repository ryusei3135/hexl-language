ファイル	中身
builder.rs	mod宣言、import、NEXT_VAR_NODE_IS_GET_ADDR、IR::new、builder()、テスト用ヘルパー(test_span、parse_test_source)
builder/memory.rs	stack_counter、gen_mem_def_var、mem_var_tests
builder/syntax.rs	gen_inst、gen_stmt_ir、gen_loop_expr_ir、gen_match_expr_ir、gen_ternary_branch_ir、match_expr_ir_tests
builder/expr.rs	gen_expr_ir、left_right_pair、build_expr_inst、ternary_is_select、is_number_only、is_mem_free、unary_expr_ir_tests
builder/misc.rs	Self型の解決(4関数)、expand_*_methods、unwrap_or_report、next_pattern_label、テスト用の取得関数、self_ty_testsほか2つのテスト