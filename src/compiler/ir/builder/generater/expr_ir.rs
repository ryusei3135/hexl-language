//! 式のIR生成: `gen_expr_ir`と、二項演算・三項演算子の判定ヘルパー

use super::*;
use crate::compiler::ast::TyNode;

impl IR {
    #[inline(always)]
    pub(in crate::compiler::ir) fn left_right_pair<M: ResultMod>(
        &mut self,
        node: &(Box<ast::Expr>, Box<ast::Expr>),
        expect_byte: &types::Size,
    ) -> Result<(usize, usize), err::ErrKind> {
        // 型が決まっていない即値は、もう片方のオペランドの型(定義元の型)にする
        let (ls_ty, rs_ty) = self.operand_expect_tys(&node.0, &node.1, expect_byte);
        Ok((
            self.gen_expr_ir::<M>(&node.0, Some(&ls_ty))?,
            self.gen_expr_ir::<M>(&node.1, Some(&rs_ty))?,
        ))
    }

    /// 指定した種類の式のノードを作成する
    #[inline(always)]
    pub(in crate::compiler::ir) fn build_expr_inst<M: ResultMod>(
        &mut self,
        node: &(Box<ast::Expr>, Box<ast::Expr>),
        expect_byte: &types::Size,
        kind: inst::ExprKind,
    ) -> Result<inst::Inst, err::ErrKind> {
        let pair = self.left_right_pair::<M>(node, &expect_byte)?;
        Ok(inst::ExprInst {
            dst: self.id_counter,
            ls: pair.0,
            rs: pair.1,
            kind,
        }
        .new())
    }

    /// `expect_byte`は期待する型。`None`の場合は未確定で、
    /// `Assign`ノードが来たら代入先の変数の型で確定させる
    /// (それ以外のノードでは`DD`にフォールバックする)
    ///
    /// `M`はエラーの扱い方(`src/ir/builder/result_mod.rs`)
    /// - `ReturnErr`: `Err`を返す
    /// - `PanicErr`: その場でpanicする
    pub(in crate::compiler::ir) fn gen_expr_ir<M: ResultMod>(
        &mut self,
        expr: &ast::Expr,
        expect_byte: Option<&types::Size>,
    ) -> Result<usize, err::ErrKind> {
        // 前置の単項演算子(`+a` `-a` `!a` `~a` `++a` `--a`)は、
        // 既存のノードに展開してから生成し直す
        // (`src/ir/builder/expr_node.rs`の`desugar_unary`)
        if let Some(expanded) = self.desugar_unary::<M>(&expr, expect_byte)? {
            return self.gen_expr_ir::<M>(&expanded, expect_byte);
        }
        // `Type::Mem(value)`がバリアント型の初期化なら、`InitVariant`に直す
        // (`src/ir/builder/variant_bind.rs`)
        let expr = self.scope_call_to_variant_init(expr);
        // `src/ir/builder/expr_node.rs`
        let resolved = self.resolve_expect_byte::<M>(&expr, expect_byte)?;
        let expect_byte = &resolved;
        self.expr_counter += 1;
        // 配列/ポインタのアクセスやポインタの加算の範囲チェック
        // (`src/ir/checker/access_mem.rs`)
        // 添字が可変な配列アクセスのときは、範囲外のときの分岐を
        // IRに差し込むためのガードが返る
        let arr_guard = M::handle(self.check_mem_access::<M>(&expr))?;
        // `&&` / `||`、`bool`型への比較など、0か1の値が必要な式は`Set*`命令にする
        if Self::needs_setcc(&expr, Some(expect_byte)) {
            return self.gen_setcc_ir::<M>(&expr);
        }
        let inst = match &expr.node {
            // ポインタ関係
            ast::ExprNode::GetAddress(target) => {
                self.expr_is_ptr_val = NEXT_VAR_NODE_IS_GET_ADDR;
                let idx = self.gen_expr_ir::<M>(target, Some(&expect_byte))?;
                inst::Inst::GetAddress(idx)
            }
            ast::ExprNode::ConnectAddr(target) => {
                let idx = self.gen_expr_ir::<M>(target, Some(&expect_byte))?;
                inst::Inst::Pointer(idx)
            }
            // `(ty)expr`: 組み込み型へのキャスト
            // `src/ir/builder/expr_node.rs`
            ast::ExprNode::Cast((ty, target)) => self.build_cast_inst::<M>(&ty, &*target)?,
            ast::ExprNode::Inc(var) => self.build_inc_dec_expr::<M>(&*var, &inst::ExprKind::Inc)?,
            ast::ExprNode::Dec(var) => self.build_inc_dec_expr::<M>(&*var, &inst::ExprKind::Dec)?,

            ast::ExprNode::Add(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::Add)?
            }
            ast::ExprNode::Sub(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::Sub)?
            }
            ast::ExprNode::Mul(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::Mul)?
            }
            ast::ExprNode::Div(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::Div)?
            }
            ast::ExprNode::Surplus(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::Surplus)?
            }
            ast::ExprNode::LessThen(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::LessThen)?
            }
            ast::ExprNode::GreaterThen(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::GreaterThen)?
            }
            ast::ExprNode::LessThenEq(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::LessThenEq)?
            }
            ast::ExprNode::GreaterThenEq(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::GreaterThenEq)?
            }
            ast::ExprNode::Equal(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::Equal)?
            }
            ast::ExprNode::NotEq(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::NotEq)?
            }
            ast::ExprNode::AndCalcul(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::AndCalcul)?
            }
            ast::ExprNode::XorCalcul(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::XorCalcul)?
            }
            ast::ExprNode::OrCalcul(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::OrCalcul)?
            }
            ast::ExprNode::LShift(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::LShift)?
            }
            ast::ExprNode::RShift(node) => {
                self.build_expr_inst::<M>(node, &expect_byte, inst::ExprKind::RShift)?
            }
            ast::ExprNode::Number(value) => {
                inst::Inst::gen_num(&value, &expect_byte, self.id_counter)
            }
            ast::ExprNode::Float(value) => {
                inst::Inst::gen_float(&value, &expect_byte, self.id_counter)
            }
            ast::ExprNode::Sizeof(ty) => {
                let byte_size = self.byte_size_of(ty)?;
                inst::Inst::gen_num(&byte_size.to_string(), &types::Size::UDQ, self.id_counter)
            }
            ast::ExprNode::Assign(assign_node) => {
                let var_attr = self.var_tree.is_mut(&assign_node.name);
                // `src/ir/builder/expr_node.rs`
                let result = self.assign_expr_node::<M>(assign_node, &expect_byte, &var_attr);
                M::handle(result)?
            }
            ast::ExprNode::Str(value) => inst::Inst::Str {
                dst: self.id_counter,
                value: value.to_string(),
            },
            // `cond ? left : right`
            // 条件式がメモリに触れず、両辺が数字のみの式なら`ExprKind::Ternary`の1命令にする。
            // それ以外は、副作用やメモリアクセスを選ばれた側でだけ実行するため、
            // `cond`と同じ形の条件分岐(ラベルとジャンプ)に展開する
            ast::ExprNode::Ternary(node) => {
                let (cond, left, right) = node;
                // この条件がtrueなら必ず三項演算子
                if Self::ternary_is_select(&cond, &left, &right) {
                    // `ternary_expr`が`self.ternatry = Some(true)`にして、(cond, left, right)を生成する
                    let idxs = self.ternary_expr::<M>(&cond, &left, &right, expect_byte)?;
                    inst::ExprInst {
                        dst: idxs.0,
                        ls: idxs.1,
                        rs: idxs.2,
                        kind: inst::ExprKind::Ternary,
                    }
                    .new()
                } else {
                    self.ternatry = None;
                    return self.gen_ternary_branch_ir::<M>(cond, left, right, expect_byte);
                }
            }
            // 配列を初期化する
            ast::ExprNode::Array(init_nodes) => {
                // `src/ir/builder/expr_node.rs`
                self.init_array_node::<M>(init_nodes.to_vec(), &expect_byte)?
            }
            // 配列にアクセスする
            // else_idxはchecker/access_mem.rsでチェック済み
            ast::ExprNode::RefArray {
                name, dst, index, ..
            } => {
                // 配列変数を使ったことを記録(`src/ir/counter_mnger.rs`)
                self.count_var_use(&name);
                // 添字が可変: `if lo <= idx < hi { arr[idx] } else { arr[else_idx] }`
                // の条件分岐を差し込む
                // `src/ir/insert/array_guard.rs`
                if let Some(guard) = arr_guard {
                    return self.insert_arr_guard::<M>(guard, &*dst, &expect_byte);
                }
                // 添字が`const`/即値: 静的検査済みなのでそのまま
                // `src/ir/builder/expr_node.rs`
                self.ref_array_node::<M>(&*dst, &*index, &name, &expect_byte)?
            }
            // ポインタの中身
            ast::ExprNode::DefVar(var) => {
                let var_attr: parse::VarMutAttr = var.var_attr.clone();
                // `src/ir/builder/expr_node.rs`
                let result = self.def_var_node::<M>(var.clone(), &expect_byte, &var_attr);
                M::handle(result)?
            }
            ast::ExprNode::CallFunc(meta_data) => {
                let result = self.gen_call_fn_ir::<M>(None, &meta_data, None);
                M::handle(result)?
            }
            ast::ExprNode::Var(var_name) => {
                // 変数(ローカル変数/引数)を使ったことを記録
                // (`src/ir/counter_mnger.rs`)
                self.count_var_use(&var_name);

                let binding: TyNode = self.var_tree.get_ty_node(&var_name).unwrap();
                // 組み込み型ならそのまま、そうでなければ(構造体/バリアント型)
                // `struct_tree`/`variant_tree`から探して型を決める
                let mut ty: Size = match Size::try_from(binding.clone()) {
                    Ok(ty) => ty,
                    Err(_) => {
                        let result = self.try_size_or_emit_struct(&binding);
                        M::handle(result)?
                    }
                };
                ty = if self.expr_is_ptr_val {
                    // 型をwrapしたのでフラグを閉じる
                    self.expr_is_ptr_val = false;
                    ty.wrap_get_addr()
                } else {
                    ty
                };
                // `src/ir/ty_checker/var_ty.rs`
                self.check_var_ty(&ty, &expect_byte, &expr.span);

                return Ok(match self.var_tree.get(&var_name) {
                    def_tree::VarType::Local(index) => *index,
                    def_tree::VarType::Param(param) => {
                        // 引数のノード
                        *param
                    }
                });
            }
            ast::ExprNode::Match {
                pattern,
                arms,
                arm_else,
            } => {
                return self.gen_match_expr_ir::<M>(&pattern, &arms, &arm_else);
            }
            ast::ExprNode::Loop { pattern, body } => {
                return self.gen_loop_expr_ir::<M>(pattern, &body);
            }
            // `M(val) := value`は、`cond`の条件式の中でだけ使える
            // (`gen_match_expr_ir`が展開するので、ここには来ない)
            ast::ExprNode::VariantBind(bind) => panic!(
                "`{}({}) := ..`は`cond`の条件式の中でのみ使えます",
                bind.variant, bind.bind
            ),
            // バリアント型のメンバへのアクセス: `Name::Mem`
            // - 全メンバーが型なしの(旧列挙型相当)場合は、定義順に基いた
            //   タグ(整数値)として展開する
            // - データ付きメンバーを持つ場合は、タグ付きの値として展開する
            ast::ExprNode::EnumVariant { name, variant } => {
                // `src/ir/builder/expr_node.rs`
                *M::handle(self.enum_variant_node(&name, &variant, &expect_byte))?
            }
            // 構造体の初期化: `Name { field: value, .. }`
            ast::ExprNode::InitStruct {
                is_self,
                name,
                fields,
            } => {
                let mut fields_c = fields.clone();
                // パーサーは`Name { .. }`が構造体かバリアント型かを区別できないので、
                // バリアント型の名前だった場合はここでバリアント型の初期化として扱う
                let is_variant = self.variant_tree.contains_key(&name)
                    || (*is_self
                        && self.this_is_self
                        && self
                            .var_tree
                            .get_ty_name(&name)
                            .map_or(false, |n| self.variant_tree.contains_key(&n)));
                if is_variant {
                    // `src/ir/builder/expr_node.rs`
                    let result =
                        self.init_variant_node::<M>(*is_self, &name, &mut fields_c, &expect_byte);
                    M::handle(result)?
                } else {
                    // `src/ir/builder/expr_node.rs`
                    M::handle(self.init_struct_node::<M>(&name, &mut fields_c))?
                }
            }
            // バリアント型の初期化: `Name { field: value }`
            ast::ExprNode::InitVariant {
                is_self,
                name,
                fields,
            } => {
                let mut fields_c = fields.clone();
                // `src/ir/builder/expr_node.rs`
                let result =
                    self.init_variant_node::<M>(*is_self, &name, &mut fields_c, &expect_byte);
                M::handle(result)?
            }
            // ここでは対応する「元の変数名」が分からない文脈
            // (関数の引数や構造体フィールドの初期化式など)から
            // 呼ばれているので`None`を渡す。`let`/代入経由で呼ばれる
            // 場合は`def_var_node`/`assign_expr_node`が
            // `gen_named_expr_ir`経由で`scope_node`を直接呼び出し、
            // 変数名を渡している(`src/ir/builder/expr_node.rs`)
            ast::ExprNode::Scope { scope, target } => {
                let result = self.scope_node::<M>(&scope, &target, None, &parse::VarMutAttr::Invar);
                M::handle(result)?
            }
            ast::ExprNode::Member { scope, target } => {
                let scope_n_ref: Vec<&str> = scope.iter().map(|s| s.as_str()).collect();
                match &(*target).node {
                    ast::ExprNode::Var(name) => {
                        // `src/ir/builder/member.rs`
                        let result = self.member_is_var(&scope_n_ref, &name);
                        M::handle(result)?
                    }
                    ast::ExprNode::CallFunc(call_func_info) => {
                        // `src/ir/builder/member.rs`
                        let result = self.member_is_fn::<M>(&scope_n_ref, &call_func_info);
                        M::handle(result)?
                    }
                    // `変数名.[メンバー名 添字]`
                    ast::ExprNode::RefArray { name, index, .. } => {
                        // `src/ir/builder/member.rs`
                        M::handle(self.member_is_arr_ref(&scope_n_ref, &name, &index))?
                    }
                    t => panic!("{:?}", t), // 構造体の配列型メンバーの要素にアクセスする
                }
            }
            // ポインタが指す構造体のメンバー/メゾットへのアクセス
            // `[name].member` / `[name].method(..)`
            ast::ExprNode::PtrMember { name, target, .. } => match &(*target).node {
                ast::ExprNode::Var(member) => {
                    let result = self.member_is_var_via_ptr(&name, &member);
                    M::handle(result)?
                }
                ast::ExprNode::CallFunc(call_func_info) => {
                    let result = self.member_is_fn_via_ptr::<M>(&name, &call_func_info);
                    M::handle(result)?
                }
                ast::ExprNode::RefArray { name: member, .. } => {
                    let result = self.member_is_var_via_ptr(&name, &member);
                    M::handle(result)?
                }
                t => panic!("{:?}", t),
            },
            ast::ExprNode::RangeNode(..) => panic!(),
            // 単項演算子は、この`match`の前の`desugar_unary`で展開済み
            ast::ExprNode::Sign(..)
            | ast::ExprNode::Inversion(..)
            | ast::ExprNode::CmpNot(..)
            | ast::ExprNode::LeftInc(..)
            | ast::ExprNode::LeftDec(..) => {
                unreachable!("単項演算子は`desugar_unary`で展開されている")
            }
            // `&&` `||` `? :`など、IRの生成が未対応のノード
            other => panic!("この式のIR生成はまだ対応していません: {:?}", other),
        };

        self.ir_tree.push(inst);

        self.id_counter += 1;
        Ok(self.id_counter - 1)
    }

    // /// `gen_expr_ir::<PanicErr>`の結果を`usize`で返す
    // ///
    // /// `Result`を返すようにする前の`gen_expr_ir`と同じ使い方をする呼び出し側用
    // /// (エラーはその場でpanicするので`Err`は返らない)
    // #[inline(always)]
    // pub(in crate::compiler::ir) fn gen_expr_ir_or_panic(
    //     &mut self,
    //     expr: ast::Expr,
    //     expect_byte: Option<&types::Size>,
    // ) -> usize {
    //     self.gen_expr_ir::<PanicErr>(expr, expect_byte).unwrap()
    // }

    /// 三項演算子を`ExprKind::Ternary`の1命令にできるか
    ///
    /// 両辺は評価しても副作用が無い必要がある(`Ternary`は両辺を先に評価するため)
    /// - 左右の式が、数字のみの式
    /// - 条件式が、メモリにアクセスしない式
    pub(in crate::compiler::ir) fn ternary_is_select(
        cond: &ast::Expr,
        left: &ast::Expr,
        right: &ast::Expr,
    ) -> bool {
        Self::is_number_only(left) && Self::is_number_only(right) && Self::is_mem_free(cond)
    }

    /// 数字(整数・小数)と演算子だけでできた式か
    pub(in crate::compiler::ir) fn is_number_only(expr: &ast::Expr) -> bool {
        match &expr.node {
            ast::ExprNode::Number(_) | ast::ExprNode::Float(_) => true,
            ast::ExprNode::Add(n)
            | ast::ExprNode::Sub(n)
            | ast::ExprNode::Mul(n)
            | ast::ExprNode::Div(n)
            | ast::ExprNode::Surplus(n)
            | ast::ExprNode::LShift(n)
            | ast::ExprNode::RShift(n)
            | ast::ExprNode::AndCalcul(n)
            | ast::ExprNode::XorCalcul(n)
            | ast::ExprNode::OrCalcul(n) => {
                Self::is_number_only(&n.0) && Self::is_number_only(&n.1)
            }
            ast::ExprNode::Sign((_, target))
            | ast::ExprNode::Inversion(target)
            | ast::ExprNode::CmpNot(target) => Self::is_number_only(target),
            _ => false,
        }
    }

    /// メモリにアクセスしない式か(変数・数字・演算・比較・キャストだけ)
    ///
    /// 配列・ポインタ・メンバー・関数呼び出し・代入・`++`/`--`などが
    /// 1つでも含まれていれば`false`(安全側に倒す)
    pub(in crate::compiler::ir) fn is_mem_free(expr: &ast::Expr) -> bool {
        match &expr.node {
            ast::ExprNode::Var(_) | ast::ExprNode::Number(_) | ast::ExprNode::Float(_) => true,
            ast::ExprNode::Add(n)
            | ast::ExprNode::Sub(n)
            | ast::ExprNode::Mul(n)
            | ast::ExprNode::Div(n)
            | ast::ExprNode::Surplus(n)
            | ast::ExprNode::LShift(n)
            | ast::ExprNode::RShift(n)
            | ast::ExprNode::AndCalcul(n)
            | ast::ExprNode::XorCalcul(n)
            | ast::ExprNode::OrCalcul(n)
            | ast::ExprNode::LessThen(n)
            | ast::ExprNode::GreaterThen(n)
            | ast::ExprNode::LessThenEq(n)
            | ast::ExprNode::GreaterThenEq(n)
            | ast::ExprNode::Equal(n)
            | ast::ExprNode::NotEq(n) => Self::is_mem_free(&n.0) && Self::is_mem_free(&n.1),
            ast::ExprNode::Sign((_, target))
            | ast::ExprNode::Inversion(target)
            | ast::ExprNode::CmpNot(target) => Self::is_mem_free(target),
            ast::ExprNode::Cast((_, target)) => Self::is_mem_free(target),
            _ => false,
        }
    }
}

#[cfg(test)]
mod unary_expr_ir_tests {
    use super::*;

    fn build_func_body(src: &str) -> Vec<inst::Inst> {
        let nodes = parse_test_source(src);
        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();
        ir.test_only_get_func_body("_start").to_vec()
    }

    fn has_kind(body: &[inst::Inst], expected: inst::ExprKind) -> bool {
        body.iter().any(|inst| match inst {
            inst::Inst::Expr(e) => e.kind == expected,
            _ => false,
        })
    }

    #[test]
    fn negative_sign_expands_to_sub_from_zero() {
        let body = build_func_body("_start(): int { a: int = 10 b: int = -a ret b }");
        assert!(has_kind(&body, inst::ExprKind::Sub), "{:?}", body);
    }

    #[test]
    fn positive_sign_generates_no_extra_inst() {
        let plain = build_func_body("_start(): int { a: int = 10 b: int = a ret b }");
        let signed = build_func_body("_start(): int { a: int = 10 b: int = +a ret b }");
        assert_eq!(plain.len(), signed.len());
    }

    #[test]
    fn bit_inversion_expands_to_xor() {
        let body = build_func_body("_start(): int { a: int = 10 b: int = ~a ret b }");
        assert!(has_kind(&body, inst::ExprKind::XorCalcul), "{:?}", body);
    }

    #[test]
    fn bit_inversion_of_unsigned_uses_type_max_mask() {
        // 符号なしに`-1`は入らないので、型の最大値をマスクにする
        let body = build_func_body("_start(): uint { a: uint = 10 b: uint = ~a ret b }");
        assert!(has_kind(&body, inst::ExprKind::XorCalcul), "{:?}", body);
    }

    #[test]
    fn logical_not_expands_to_equal_zero() {
        let body = build_func_body("_start(): int { a: int = 10 b: int = !a ret b }");
        assert!(has_kind(&body, inst::ExprKind::Equal), "{:?}", body);
    }

    #[test]
    fn unary_minus_literal_in_comparison_is_typed_like_other_operand() {
        // `a < -1`の`-1`は、`a`の型(定義元の型)になる
        let body = build_func_body("_start(): int { a: int = 10 cond a < -1 { b: int = 1 } }");
        assert!(has_kind(&body, inst::ExprKind::LessThen), "{:?}", body);
    }
}
