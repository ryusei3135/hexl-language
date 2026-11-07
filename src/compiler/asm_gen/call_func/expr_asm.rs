use super::*;

impl AsmEmitter {
    #[inline(always)]
    pub(super) fn expect_jmp(&mut self, name: &str) {
        // 次のフォーマットに使うラベルの名前を予約する
        // if self.reserved_label_name.is_none() {
        self.fn_state.reserved_label_name = Some(name.to_string());
        // } else {
        //     // 予約するラベルの名前は予約するときにNoneで無ければいけない
        //     panic!("system err");
        // }
    }

    #[inline(always)]
    pub(super) fn gen_str_asm(&mut self, dst: usize, value: &str) {
        let label_name = format!("M{}", self.data_idx.to_string());
        let fmt_data = self.asm_fmt.get_str_fmt(&value, &label_name);
        self.data_sec_text.push_str(&fmt_data);
        self.fn_state.data_map.push((dst, label_name));
        self.data_idx += 1;
    }

    /// 文字列`M0`の長さを入れる静的データ`M0Len`を出力する
    ///
    /// ```text
    /// M0:    .ascii "hello world"
    /// M0Len: .quad 11
    /// ```
    /// `dst`のラベルを`M0Len`として登録するので、オペランドは`M0Len(%rip)`になる
    pub(super) fn gen_str_len_asm(
        &mut self,
        dst: usize,
        str_id: usize,
        len: usize,
        size: &types::Size,
    ) -> Result<(), err::ErrKind> {
        let str_label = self
            .static_label(str_id)
            .ok_or_else(|| err::ErrKind::asm_emit_label_not_found("string", str_id))?;
        let label_name = format!("{}Len", str_label);
        // 同じ文字列の長さを何度も渡しても、`M0Len`は1つだけ出力する
        if !self.fn_state.data_map.iter().any(|(_, l)| *l == label_name) {
            let fmt_data = self.asm_fmt.get_static_len_fmt(len, &label_name, size);
            self.data_sec_text.push_str(&fmt_data);
        }
        self.fn_state.data_map.push((dst, label_name));
        Ok(())
    }

    #[inline(always)]
    pub(super) fn gen_float_asm(&mut self, dst: usize, value: &str, size: &types::Size) {
        let label_name = format!("M{}", self.data_idx);
        let fmt_data = self.asm_fmt.get_float_fmt(value, &label_name, size);
        self.data_sec_text.push_str(&fmt_data);
        self.fn_state.data_map.push((dst, label_name));
        self.data_idx += 1;
    }

    /// 式の計算に使うレジスタ/ニーモニックのサイズを決める
    ///
    /// 基本は右辺の型。ただし右辺が数値リテラルのときは、リテラルの型(`int`=32bit)ではなく
    /// 左辺の型に合わせる。そうしないと、64bitの変数`ret`に対する`ret < 0`や`ret + 0`が
    /// `cmpl $0, %rbx`/`movl %rbx, %esi`のように、32bitの命令へ64bitのレジスタが混ざる
    /// (アセンブラの`register size mismatch`)
    fn expr_operand_size(&self, expr: &inst::ExprInst) -> Size {
        let rs_ty = self.get_expr_ty(expr.rs);
        if !matches!(self.fn_state.curr_inst[expr.rs], inst::Inst::Num { .. }) || rs_ty.is_float() {
            return rs_ty;
        }
        let ls_ty = self.get_expr_ty(expr.ls);
        if ls_ty.is_float() {
            return rs_ty;
        }
        let ls_ty = match ls_ty {
            Size::Pointer { .. } | Size::Array { .. } | Size::Struct(..) => {
                Self::value_reg_size(&ls_ty)
            }
            other => other,
        };
        if ls_ty.to_bytes() >= rs_ty.to_bytes() {
            ls_ty
        } else {
            rs_ty
        }
    }

    #[inline(always)]
    pub(super) fn gen_expr_asm(&mut self, expr: &inst::ExprInst) -> Result<(), err::ErrKind> {
        // 三項演算子(`asm_emitter/ternary.rs`)
        // - 条件(`Ternary<比較>`): `cmp`だけを出す。結果のレジスタは作らない
        // - 選ぶ命令(`Ternary`): `mov`と`cmov`で左辺か右辺を結果のレジスタへ置く
        if Self::is_ternary_cmp_kind(&expr.kind) {
            return self.gen_ternary_cmp_asm(expr);
        }
        if expr.kind == inst::ExprKind::Ternary {
            return self.gen_ternary_select_asm(expr);
        }
        // `Set<比較>`: `cmp` + `setcc`で0/1を結果のレジスタへ置く(`asm_emitter/set_cc.rs`)
        if Self::is_set_cc_kind(&expr.kind) {
            return self.gen_set_cc_asm(expr);
        }
        // 結果を置くレジスタを、使用中のレジスタ(引数・変数・他の式の
        // 結果)と`%rax`/`%rdx`を避けて確保する。
        let size = self.expr_operand_size(expr);
        let dst = self.alloc_reg(&size)?.unwrap();
        self.fn_state.reg_idx = dst.index();

        // 予約していたラベルがこの式で使われたら、この式は条件ジャンプ
        // (`cmpq $0, %rax` + `jz L0`など)を出していて、結果はフラグで渡される。
        // 結果のレジスタ(`dst`)は書き込まれず、誰にも読まれない
        let had_label = self.fn_state.reserved_label_name.is_some();
        let asm = self.format_expr_inst(&expr, Some(size.clone()))?;
        let is_branch_only = had_label && self.fn_state.reserved_label_name.is_none();

        self.asm_text.push_str(&asm);

        if is_branch_only {
            // 結果のレジスタを使用中にしない。以前は使用中のまま誰も解放せず、
            // `if`/`match`/`loop`の条件のたびにレジスタが1つずつ塞がっていた
            // (`StartScope`より前に確保されるので、`EndScope`でも解放されない)
            self.release_expr_temp(expr.ls);
            self.release_expr_temp(expr.rs);
            return Ok(());
        }

        // 式の結果を置いたレジスタを使用中として記録する
        self.used_reg.mark_used(&dst);
        self.fn_state.last_inst_idx.push((expr.dst, dst));

        // この式が使い終わった、オペランドの一時的な結果のレジスタを解放する
        self.release_expr_temp(expr.ls);
        self.release_expr_temp(expr.rs);

        // 条件の式なのに条件ジャンプを出していない(`a && b`の`andq`など、値を
        // 計算するだけの式)場合は、結果が0でなければ予約したラベルへジャンプする
        if had_label
            && self.fn_state.reserved_label_name.is_some()
            && !self.is_expr_consumed(expr.dst)
        {
            self.emit_jump_if_nonzero(dst, &size);
            self.release_expr_temp(expr.dst);
        }
        Ok(())
    }

    /// `dst`のレジスタの値が0でなければ、予約したラベル(`ExpectJmp`)へジャンプする
    /// (`cmpq $0, %rdi` + `jnz L1`)
    fn emit_jump_if_nonzero(&mut self, dst: RegisterMode, size: &Size) {
        let Some(label) = self.fn_state.reserved_label_name.take() else {
            return;
        };
        let Ok(reg_txt) = self.get_reg_txt(Some(dst.index()), size) else {
            self.fn_state.reserved_label_name = Some(label);
            return;
        };
        let tmpl = self
            .asm_fmt
            .get_opcode_tmpl("cmp_ne")
            .replace("{src1}", &reg_txt)
            .replace("{src2}", &self.asm_fmt.get_fmt_num("0"));
        let line = self.fmt_one_expr_mnemo_resize(tmpl, size, "cmp", false);
        self.asm_text.push_str(&line.replace("{label}", &label));
    }

    #[inline(always)]
    pub(super) fn gen_complie_asm(
        &mut self,
        name: &str,
        lines: &[(String, Vec<usize>)],
        asm_fmt_name: Option<&str>,
    ) -> Result<(), err::ErrKind> {
        if let Some(asm_name) = asm_fmt_name {
            if name == asm_name {
                self.deploy_inline_asm::<StackErr>(name, &lines)
            } else {
                panic!("unmatch asm name");
            }
        } else {
            self.deploy_inline_asm::<StackErr>(name, &lines)
        }
    }

    #[inline(always)]
    pub(super) fn gen_assign_var_asm(
        &mut self,
        name: &str,
        dst: usize,
        value: usize,
        this_is_self: bool,
    ) -> Result<(), err::ErrKind> {
        let is_mem_write = matches!(
            self.fn_state.curr_inst[dst],
            inst::Inst::Pointer(..)
                | inst::Inst::InsertArr { .. }
                | inst::Inst::RefStruct { .. }
                | inst::Inst::RefStructPtr { .. }
        );

        if is_mem_write {
            self.write_mem(name, dst, value, Some(&self.get_var_ty(name)?))?;
            Ok(())
        } else {
            // 通常の変数への再代入(`b = 10`など)
            self.update_value_info(&name, value);

            // レジスタに置かれている変数は、そのレジスタへそのまま書き込む。
            // (以前は`reg_idx`のレジスタへ変数を「引っ越し」させていたため、
            //  引数や他の変数のレジスタを壊したり、ループの先頭が
            //  参照するレジスタと食い違ったりしていた)
            let var_ty = self.get_var_ty(name)?;
            // レジスタが足りずスタックに置いた変数は、そのメモリへ書き込む
            if self.is_spilled_scalar(name) {
                let s = if this_is_self {
                    None
                } else {
                    var_ty.wrap_dst_size()
                };
                self.assign_spilled_var(name, |this, scratch| {
                    if var_ty.is_pointer().is_some() {
                        this.assign_val_ty_is_ptr(scratch, value, s.as_ref())
                    } else if var_ty.is_float() {
                        this.assign_val_is_float(scratch, value, &var_ty, s.as_ref())
                    } else {
                        this.assign_val_is_not_ptr(scratch, value, s.as_ref())
                    }
                })?;
                // 代入する値が式の結果だった場合、そのレジスタはもう不要
                self.release_expr_temp(value);
                return Ok(());
            }
            let current_reg = self.var_reg(name).unwrap_or_else(|| {
                if var_ty.is_float() {
                    RegisterMode::Float(self.fn_state.reg_idx)
                } else {
                    RegisterMode::Normal(self.fn_state.reg_idx)
                }
            });
            if matches!(
                self.fn_state.curr_inst[value],
                inst::Inst::Expr(..) | inst::Inst::Cast { .. }
            ) && self
                .fn_state
                .last_inst_idx
                .iter()
                .any(|(idx, reg)| *idx == value && *reg == current_reg)
            {
                return Ok(());
            }
            let s = if this_is_self {
                None
            } else {
                var_ty.wrap_dst_size()
            };

            // 代入先の変数の型(サイズ)を確認し、ポインタ型
            // であれば、専用のフォーマット(`get_ptr`)で
            // アドレスのオペランドを組み立てる
            let text = if var_ty.is_pointer().is_some() {
                self.assign_val_ty_is_ptr(current_reg, value, s.as_ref())?
            } else if var_ty.is_float() {
                self.assign_val_is_float(current_reg, value, &var_ty, s.as_ref())?
            } else {
                self.assign_val_is_not_ptr(current_reg, value, s.as_ref())?
            };

            self.asm_text.push_str(&text);
            // 代入する値が式の結果だった場合、そのレジスタはもう不要
            self.release_expr_temp(value);
            Ok(())
        }
    }

    #[inline(always)]
    pub(super) fn gen_ret_asm(
        &mut self,
        fn_ret_ty: SelfPtrInfo,
        idx: usize,
        fn_name: &str,
    ) -> Result<(), err::ErrKind> {
        // `_start`はOSから直接呼ばれるエントリーポイントであり、
        // `call`で呼ばれたわけではないため`ret`で戻ることができない。
        // `_start`の中に明示的な`return`(`Inst::Ret`)が書かれていた
        // 場合も、通常の`leave; ret`ではなく`sys_exit`で終了させる。
        let _is_start = fn_name == "_start";

        if fn_ret_ty.is_none() {
            if let Some(struct_idx) = self.resolve_struct_idx(idx) {
                let mem = match self.inst_at(struct_idx) {
                    inst::Inst::Struct { mem, .. } => mem,
                    _ => panic!("構造体の戻り値を解決できません"),
                };
                let t = self.emit_struct_init_asm(mem, true)?;
                self.asm_text.push_str(t.as_str());
                // if is_start {
                //     self.asm_text.push_str(
                //         &self.gen_exit_syscall_asm()
                //     );
                // } else {
                if !_is_start {
                    self.asm_text.push_str(CALLEE_RESTORE_MARK);
                }
                self.asm_text
                    .push_str(self.asm_fmt.func_frame_end().as_str());
                self.asm_text.push_str(&self.asm_fmt.get_ret());
                // }
                return Ok(());
            }
        }
        let mnemonic = if self.fn_state.curr_inst[idx].is_float(&self.fn_state.curr_inst) {
            "float_mov"
        } else {
            "mov"
        };
        let ret_asm = self.format_line(mnemonic, Some(0), idx, None, fn_ret_ty)?;
        self.asm_text.push_str(&ret_asm);
        // if is_start {
        //     self.asm_text
        //         .push_str(
        //             &self.gen_exit_syscall_asm()
        //         );
        // } else {
        if !_is_start {
            self.asm_text.push_str(CALLEE_RESTORE_MARK);
        }
        self.asm_text
            .push_str(self.asm_fmt.func_frame_end().as_str());
        self.asm_text.push_str(&self.asm_fmt.get_ret());
        // }
        Ok(())
    }
}
