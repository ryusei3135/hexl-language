//! 構文(文・制御構文)のIR生成: 文のループ、`return`/`break`、
//! `loop`、`match`、三項演算子の分岐展開

use super::*;

impl IR {
    pub(in crate::compiler::ir) fn gen_inst<M: ResultMod>(
        &mut self,
        node: &Body,
    ) -> Result<usize, err::ErrKind> {
        for stmt in node {
            self.current_span = Rc::clone(&stmt.span);
            self.expr_counter = 0;
            self.constract_flag.reset();
            match stmt.get_node().clone() {
                Group2Node::Expr(expr) => {
                    let _ = self.gen_expr_ir::<M>(&expr, None)?;
                }
                Group2Node::Stmt(stmt) => {
                    let _ = self.gen_stmt_ir::<M>(stmt)?;
                }
                // inlineアセンブラ
                Group2Node::CompleSyntax((name, lines)) => {
                    self.inline_proc::<M>(&lines, &name)?;
                }
                _ => {}
            }
        }
        Ok(self.id_counter)
    }

    /// 文のノードを生成
    pub(super) fn gen_stmt_ir<M: ResultMod>(
        &mut self,
        stmt: StmtNode,
    ) -> Result<usize, err::ErrKind> {
        let node = match stmt {
            StmtNode::Return(expr) => {
                let func_ret_ty = self.func_ret_ty.as_ref().ok_or_else(|| {
                    crate::GenUndefErrResult!(UndefFuncRetTy, "none".to_string(), None)
                })?;
                let ret_size = self.size_of(&func_ret_ty);
                let idx = self.gen_expr_ir::<M>(&expr, Some(&ret_size))?;
                inst::Inst::Ret(idx)
            }
            StmtNode::Continue => {
                let (continue_label, _) = self
                    .loop_labels
                    .last()
                    .expect("continueはloopの中でのみ使用できます");
                inst::Inst::Jmp(format!("L{}", continue_label))
            }
            StmtNode::Break(span) => {
                let result = self.end_scope(false, &span);
                self.unwrap_or_report(result);
                let (_, break_label) = self
                    .loop_labels
                    .last()
                    .expect("breakはloopの中でのみ使用できます");
                inst::Inst::Jmp(format!("L{}", break_label))
            }
        };

        self.ir_tree.push(node);
        self.id_counter += 1;
        Ok(self.id_counter)
    }

    #[inline(always)]
    pub(super) fn gen_loop_expr_ir<M: ResultMod>(
        &mut self,
        pattern: &Option<Box<ast::Expr>>,
        body: &Body,
    ) -> Result<usize, err::ErrKind> {
        // 本体の処理中に`current_span`が書き換わるので、先に控えておく
        let scope_span = Rc::clone(&self.current_span);
        // 反復処理が始まる場所を作成
        let start = self.pattern_labels;
        self.pattern_labels += 1;
        let end = self.pattern_labels;
        self.pattern_labels += 1;
        crate::push_jmp_code!(self, Block, start.clone());

        // ループする条件の作成
        if let Some(expr) = pattern {
            // 条件式の生成中に内側の制御構文がラベルを発番しても、
            // 条件ジャンプの飛び先が変わらないよう先に予約する。
            let condition = self.pattern_labels;
            self.pattern_labels += 1;
            crate::push_jmp_code!(self, ExpectJmp, &condition);
            let _ = self.gen_expr_ir::<M>(expr, None)?;
            // もし条件がfalseならendまでジャンプ
            crate::push_jmp_code!(self, Jmp, &end);
            // 条件がtrueのときジャンプする場所
            crate::push_jmp_code!(self, Block, &condition);
        }
        self.loop_labels.push((start.clone(), end.clone()));
        self.begin_scope();
        self.gen_inst::<M>(&body)?;
        self.end_block_scope(&scope_span);
        self.loop_labels.pop();
        crate::push_jmp_code!(self, Jmp, &start);
        crate::push_jmp_code!(self, Block, &end);
        Ok(self.id_counter - 1)
    }

    #[inline(always)]
    pub(super) fn gen_match_expr_ir<M: ResultMod>(
        &mut self,
        pattern: &Option<Box<ast::Expr>>,
        arms: &[MatchArm],
        arm_else: &Option<Body>,
    ) -> Result<usize, err::ErrKind> {
        // 本体の処理中に`current_span`が書き換わるので、先に控えておく
        let scope_span = Rc::clone(&self.current_span);

        // 各armの条件式を作成する
        // - `pattern`(matchに与えられた値)がある場合は、
        //   その値とarmのパターンを比較する式(値を与えるパターン)にする
        // - 無い場合は、armのパターンをそのまま真偽値の条件として使う
        //   (真偽値/何も与えないパターン)
        let build_cond = |arm_pattern: &ast::Expr| -> ast::Expr {
            if let Some(target) = pattern.as_ref() {
                ast::Expr::new(
                    ast::ExprNode::Equal((target.clone(), Box::new(arm_pattern.clone()))),
                    Rc::clone(&arm_pattern.span),
                )
            } else {
                arm_pattern.clone()
            }
        };

        // 各アームの本体に対応する、一意なラベル番号を先にまとめて
        // 発番しておく。
        //
        // (以前は`ExpectJmp`側に`self.pattern_labels`を、
        //  `Block`側に`self.jmp_labels`をそのまま使い、しかも
        //  どちらもループの中で一度もインクリメントしていなかった
        //  ため、全てのアームが同じラベル名(例: `L0`)を共有して
        //  しまい、`jz`の飛び先も本体のラベルも区別できなくなる
        //  バグがあった)
        let arm_labels: Vec<usize> = arms.iter().map(|_| self.next_pattern_label()).collect();

        // 全アームの処理が終わった後にジャンプする、一意な終了ラベル
        let end_label = self.next_pattern_label();

        // 各アームの条件式(一致すれば、対応するアームのラベルへジャンプする)
        // `M(val) := value`のアームは、`value.label == tag_of(M)`に展開し、
        // `val`の定義(`val = value.value`)はアームの本体の先頭へ置くので、
        // その情報(`binds`)を覚えておく
        let mut binds: Vec<Option<variant_bind::VariantBindInfo>> = Vec::new();
        for (arm, label) in arms.iter().zip(arm_labels.iter()) {
            crate::push_jmp_code!(self, ExpectJmp, label);
            if let ast::ExprNode::VariantBind(bind) = &(*arm.pattern).node {
                // `src/ir/builder/variant_bind.rs`
                let result = self.variant_bind_cond(bind);
                binds.push(Some(self.unwrap_or_report(result)));
            } else {
                let cond = build_cond(&arm.pattern);
                self.gen_expr_ir::<M>(&cond, None)?;
                binds.push(None);
            }
        }

        // 契約(`must`)の状態は、分岐ごとに分岐の前の状態から始める。
        // (ある分岐で終了した変数を、別の分岐でも終了できるようにするため。
        //  分岐の間で契約が食い違っていないかは、`checker/constract/branch.rs`が確認する)
        // `ret`/`break`で必ず抜ける分岐で終了した変数は、分岐の後には影響しないので、
        // 抜けない分岐で終了した変数だけを、全分岐を作り終えたあとに終了済みにする
        let life_before = self.var_tree.snapshot_life();
        let mut ended_after: Vec<String> = Vec::new();

        // どの条件にも一致しなかった場合の処理(else)
        // (elseが無い場合でも、そのまま素通りして終了ラベルへ進む)
        if let Some(arm) = arm_else.clone() {
            self.begin_scope();
            self.gen_inst::<M>(&arm)?;
            self.end_block_scope(&scope_span);
            self.finish_branch_life(&life_before, &arm, &mut ended_after);
        }
        crate::push_jmp_code!(self, Jmp, &end_label);

        // 各アームの処理内容を作成する
        for ((arm, label), bind) in arms.iter().zip(arm_labels.iter()).zip(binds.iter()) {
            // このアーム専用のラベル(条件が一致した場合の飛び先)
            crate::push_jmp_code!(self, Block, label);
            self.begin_scope();
            // `M(val) := value`の`val`を定義する(`val = value.value`)
            if let Some(info) = bind {
                let result = self.variant_bind_define_var(info);
                self.unwrap_or_report(result);
            }
            self.gen_inst::<M>(&arm.body)?;
            self.end_block_scope(&scope_span);
            self.finish_branch_life(&life_before, &arm.body, &mut ended_after);
            crate::push_jmp_code!(self, Jmp, &end_label);
        }
        // 分岐の後の契約の状態
        self.var_tree.mark_life_ended(&ended_after);

        // 条件が終了したときにジャンプする場所を指定
        crate::push_jmp_code!(self, Block, end_label);
        Ok(self.id_counter - 1)
    }

    /// 分岐の本体を作り終えたあとに呼ぶ。契約の状態を分岐の前(`before`)へ戻し、
    /// 本体が`ret`/`break`で抜けないなら、その本体で終了した変数を`ended_after`に積む
    fn finish_branch_life(
        &mut self,
        before: &std::collections::HashMap<String, crate::compiler::ir::def_tree::VarLife>,
        body: &Body,
        ended_after: &mut Vec<String>,
    ) {
        if !Self::body_always_exits(body) {
            for name in self.var_tree.ended_since(before) {
                if !ended_after.contains(&name) {
                    ended_after.push(name);
                }
            }
        }
        self.var_tree.restore_life(before);
    }

    /// 三項演算子を条件分岐に展開する(`gen_match_expr_ir`と同じ形)
    ///
    /// ```text
    ///   ExpectJmp(then)
    ///   <cond>
    ///   <right>        条件が偽のとき
    ///   Jmp(end)
    ///   Block(then)
    ///   <left>         条件が真のとき
    ///   Jmp(end)
    ///   Block(end)
    /// ```
    /// `ternary_expr`と違い、`left`/`right`は選ばれたときだけ評価される。
    /// 戻り値は、最後に生成した命令の番号
    pub(super) fn gen_ternary_branch_ir<M: ResultMod>(
        &mut self,
        cond: &Box<ast::Expr>,
        left: &Box<ast::Expr>,
        right: &Box<ast::Expr>,
        expect_byte: &types::Size,
    ) -> Result<usize, err::ErrKind> {
        // 中の式が発番するラベルと重ならないよう、先にまとめて発番する
        let then_label = self.next_pattern_label();
        let end_label = self.next_pattern_label();

        // 条件が真なら`then_label`へ(`ExpectJmp`は条件式より前に置く)
        crate::push_jmp_code!(self, ExpectJmp, &then_label);
        let _ = self.gen_expr_ir::<M>(cond, None)?;

        // 条件が偽のとき
        self.begin_scope();
        let _ = self.gen_expr_ir::<M>(right, Some(expect_byte))?;
        self.end_block_scope(&(cond).span);
        crate::push_jmp_code!(self, Jmp, &end_label);

        // 条件が真のとき
        crate::push_jmp_code!(self, Block, &then_label);
        self.begin_scope();
        let _ = self.gen_expr_ir::<M>(left, Some(expect_byte))?;
        self.end_block_scope(&(*cond).span);
        crate::push_jmp_code!(self, Jmp, &end_label);

        // 合流
        crate::push_jmp_code!(self, Block, &end_label);
        Ok(self.id_counter - 1)
    }
}

#[cfg(test)]
mod match_expr_ir_tests {
    use super::*;
    use std::collections::HashSet;

    fn build_func_body(src: &str) -> Vec<inst::Inst> {
        let nodes = parse_test_source(src);
        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();
        ir.test_only_get_func_body("_start").to_vec()
    }

    #[test]
    fn check_match_bool_generates_equal_cmp() {
        // 1. 真偽値(比較式)を与えるパターンは、その式がそのまま
        //    条件分岐の判定に使われる (単純なif/else)
        let body = build_func_body(
            "_start(): int {
                a: int = 10
                cond a == 10 {
                    b: int = 1
                } else {
                    b: int = 0
                }
            }",
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Expr(inst::ExprInst {
                    kind: inst::ExprKind::Equal,
                    ..
                })
            )),
            "`a == 10` の比較命令(Equal)が生成される必要がある: {:?}",
            body
        );
    }

    #[test]
    fn logical_and_generates_boolean_ir() {
        let body = build_func_body(
            "_start(): bool {
                a: int = 1
                b: int = 2
                ret a && b
            }",
        );

        assert_eq!(
            body.iter()
                .filter(|inst| matches!(
                    inst,
                    inst::Inst::Expr(inst::ExprInst {
                        kind: inst::ExprKind::SetNotEq,
                        ..
                    })
                ))
                .count(),
            2,
            "論理演算のオペランドを真偽値に変換する必要がある: {:?}",
            body
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Expr(inst::ExprInst {
                    kind: inst::ExprKind::AndCalcul,
                    ..
                })
            )),
            "&& のIR(AndCalcul)が生成される必要がある: {:?}",
            body
        );
    }

    #[test]
    fn logical_or_generates_boolean_ir() {
        let body = build_func_body(
            "_start(): bool {
                a: int = 1
                b: int = 2
                ret a < b || b < a
            }",
        );

        assert_eq!(
            body.iter()
                .filter(|inst| matches!(
                    inst,
                    inst::Inst::Expr(inst::ExprInst {
                        kind: inst::ExprKind::SetLessThen,
                        ..
                    })
                ))
                .count(),
            2,
            "|| の比較オペランドはそれぞれ真偽値へ変換される必要がある: {:?}",
            body
        );
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Expr(inst::ExprInst {
                    kind: inst::ExprKind::OrCalcul,
                    ..
                })
            )),
            "|| のIR(OrCalcul)が生成される必要がある: {:?}",
            body
        );
    }

    #[test]
    fn match_arm_block_labels_are_unique() {
        let body = build_func_body(
            "_start(): int {
                cond 1 {
                    1 => { a: int = 1 }
                    2 => { a: int = 2 }
                    else => { a: int = 0 }
                }
                ret 1
            }",
        );

        let labels: Vec<String> = body
            .iter()
            .filter_map(|instruction| match instruction {
                inst::Inst::Block(label) => Some(label.clone()),
                _ => None,
            })
            .collect();
        let unique_labels: HashSet<&String> = labels.iter().collect();

        assert_eq!(
            labels.len(),
            unique_labels.len(),
            "matchのアーム本体ラベルが重複している: {:?}",
            labels
        );
    }

    const VARIANT_BIND_SRC: &str = "variant V { A B(int) }
        _start() {
            v: V = V::B(10)
            cond B(val) := v {
                x: int = val
            } else {
                x: int = 0
            }
        }";

    #[test]
    fn check_variant_bind_expands_to_label_compare() {
        // `B(val) := v` は `v.label == tag_of(B)` の比較に展開される
        let body = build_func_body(VARIANT_BIND_SRC);
        let label_pos = body
            .iter()
            .position(|inst| {
                matches!(
                    inst,
                    inst::Inst::RefStruct { src, pos: 0, size: types::Size::DD } if src == "v"
                )
            })
            .expect("`v.label`(タグの読み出し)が生成される必要がある");
        // `B`は定義順で1番目(`A`が0)なので、タグは1
        assert!(
            matches!(
                &body[label_pos + 1],
                inst::Inst::Num { value, .. } if value == "1"
            ),
            "タグ(1)との比較が続く必要がある: {:?}",
            body
        );
        assert!(
            matches!(
                &body[label_pos + 2],
                inst::Inst::Expr(inst::ExprInst {
                    kind: inst::ExprKind::Equal,
                    ..
                })
            ),
            "`==`の比較命令が続く必要がある: {:?}",
            body
        );
    }

    #[test]
    fn check_variant_bind_defines_val_from_payload() {
        // 条件が真のときの本体の先頭で、`val = v.value`(タグの後ろのペイロード)が
        // 定義される。それは本体の中の`x: int = val`より前にある
        let body = build_func_body(VARIANT_BIND_SRC);
        let payload_pos = body
            .iter()
            .position(|inst| {
                matches!(
                    inst,
                    inst::Inst::RefStruct { src, pos, size: types::Size::DD }
                        if src == "v" && *pos == types::VARIANT_TAG_BYTES
                )
            })
            .expect("`v.value`(ペイロードの読み出し)が生成される必要がある");
        let val_pos = body
            .iter()
            .position(|inst| matches!(inst, inst::Inst::Mov { name: Some(n), .. } if n == "val"))
            .expect("`val`の定義(Mov)が生成される必要がある");
        assert!(payload_pos < val_pos, "{:?}", body);
        // `val`の定義は、`ExpectJmp`の先(アームのラベルの後ろ)にある
        let arm_block = body
            .iter()
            .position(|inst| matches!(inst, inst::Inst::Block(_)))
            .expect("アームのラベル(Block)が必要");
        assert!(arm_block < payload_pos, "{:?}", body);
    }

    #[test]
    fn check_variant_ctor_defined_after_use() {
        // バリアント型が使う関数より後ろに定義されていても、
        // `E::b(10)`は関数呼び出しではなく、バリアント型の初期化になる
        let nodes =
            parse_test_source("_start(): int { e: E = E::b(10) ret 1 } variant E { a b(int) c }");
        let mut ir = new_test_ir();
        ir.builder::<crate::compiler::ir::result_mod::PanicErr>(&nodes)
            .unwrap();
        let body = ir.test_only_get_func_body("_start").to_vec();
        assert!(
            body.iter().any(|inst| matches!(
                inst,
                inst::Inst::Variant {
                    tag: 1,
                    value: Some(_),
                    ..
                }
            )),
            "{:?}",
            body
        );
    }

    #[test]
    fn check_variant_bind_with_explicit_ty() {
        // `B(val): int = v` も同じIRになる
        let infer = build_func_body(VARIANT_BIND_SRC);
        let explicit = build_func_body(&VARIANT_BIND_SRC.replace("B(val) := v", "B(val): int = v"));
        assert_eq!(infer, explicit);
    }
}