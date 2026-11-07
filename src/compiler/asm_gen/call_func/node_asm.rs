use super::*;

impl AsmEmitter {
    /// IRのノード1つぶんのアセンブリを生成する
    ///
    /// 生成できないノードは`Err`を返す(`build_fn_process`が`err_pool`へ入れる)
    pub(super) fn gen_node_asm(
        &mut self,
        node_idx: usize,
        node: &inst::Inst,
        ctx: &FnGenCtx,
    ) -> Result<(), err::ErrKind> {
        // 退避した変数のうち、このノードが参照するものを復元する(`lazy_spill.rs`)
        self.prepare_node_spills::<StackErr>(node)?;

        match node {
            inst::Inst::ExpectJmp(name) => self.expect_jmp(name),
            // 生成しない変数の初期値だけを構成する式・定数は生成しない
            // (`unused_var_nodes`)
            inst::Inst::Expr(_)
            | inst::Inst::Cast { .. }
            | inst::Inst::Float { .. }
            | inst::Inst::Str { .. }
            | inst::Inst::StrLen { .. }
                if self.fn_state.unused_var_nodes.contains(&node_idx) => {}
            // `asm_emitter/cast.rs`
            inst::Inst::Cast { src, after } => self.gen_cast_asm(node_idx, *src, after)?,
            inst::Inst::Str { dst, value } => self.gen_str_asm(*dst, value),
            inst::Inst::StrLen {
                dst,
                str_id,
                len,
                size,
            } => self.gen_str_len_asm(*dst, *str_id, *len, size)?,
            inst::Inst::Expr(expr) => self.gen_expr_asm(expr)?,
            inst::Inst::Num { .. } => {}
            inst::Inst::Float { dst, value, size } => self.gen_float_asm(*dst, value, size),
            inst::Inst::InitArr(_) => {}
            inst::Inst::Block(name) => {
                self.asm_text.push_str(&self.asm_fmt.get_label_fmt(name));
                // 条件付きの反復処理の終了位置なら、条件に使ったレジスタを解放する
                self.release_loop_cond_regs(node_idx);
            }
            inst::Inst::StartScope => self.start_scope_asm(),
            inst::Inst::EndScope => self.end_scope_asm(),
            inst::Inst::Comple { name, lines } => {
                self.gen_complie_asm(name, lines, ctx.asm_fmt_name.as_deref())?
            }
            inst::Inst::Jmp(name) => {
                // 直前が無条件ジャンプ(`break`の`jmp L1`など)なら、
                // 次のラベルまで到達できないので`jmp`を生成しない
                let after_jmp = self
                    .asm_text
                    .lines()
                    .rev()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .is_some_and(|l| self.asm_fmt.is_jmp_line(l));
                if !after_jmp {
                    self.asm_text.push_str(&self.asm_fmt.get_jmp(name));
                }
            }
            inst::Inst::AssignVar { name, dst, value } => {
                self.gen_assign_var_asm(name, *dst, *value, ctx.this_is_self)?
            }
            inst::Inst::Ret(idx) => {
                self.gen_ret_asm(ctx.fn_ret_ty, *idx, ctx.fn_name)?;
                // `ret`の後は次のラベルまで到達しない。退避は捨てる
                self.fn_state.spilled_regs.clear();
            }
            inst::Inst::Mov {
                name,
                size,
                dst,
                src,
            } => {
                // 使用回数が0回の変数(`unused_var_nodes`)は生成しない
                if !self.fn_state.unused_var_nodes.contains(&node_idx) {
                    self.gen_mov_node_asm(
                        name.as_deref(),
                        size,
                        *dst,
                        *src,
                        ctx.returned_struct_idx,
                    )?
                }
            }
            // メモリに配置されている値の生成
            inst::Inst::MemoryValue(mem_value) => self.mem_val_ir(mem_value)?,
            inst::Inst::Param(_) => self.gen_param_asm(node),
            inst::Inst::CallFunc(meta_data) => self.gen_call_node_asm(node_idx, meta_data)?,
            _ => {}
        }
        Ok(())
    }

    /// `Inst::Mov`(変数の定義)のアセンブリを生成する
    fn gen_mov_node_asm(
        &mut self,
        name: Option<&str>,
        size: &types::Size,
        dst: usize,
        src: usize,
        returned_struct_idx: Option<usize>,
    ) -> Result<(), err::ErrKind> {
        // 戻り値として返される構造体の初期化かどうか。
        //
        // `returned_struct_idx`が`None`(=この関数は構造体を返さない)
        // の場合に、`resolve_struct_idx(dst)`も`None`(=`dst`が
        // `a: A = A::new()`のようなコンストラクタ呼び出しで、
        // `Inst::Struct`を直接指していない)だと、`None == None`で
        // 「返される構造体」と誤判定されていた。その結果
        // `mov_value_ir`が呼ばれず、変数`a`が`var_hash_map`へ
        // 登録されないまま、以降の参照で「存在しない変数」に
        // なってしまっていた。
        // そのため、構造体が実際に返される場合(`Some`)のみ比較する
        let is_returned_struct = matches!(size, types::Size::Struct(..))
            && returned_struct_idx.is_some()
            && returned_struct_idx == self.resolve_struct_idx(dst);
        if !is_returned_struct {
            self.mov_value_ir(size, dst, src, name, Some(size))?;
            // 定義した変数のレジスタは新しい値になる
            if let Some(reg) = name.and_then(|n| self.var_reg(n)) {
                self.fn_state.spilled_regs.remove(&reg);
            }
        }
        Ok(())
    }

    /// `Inst::Param`(関数の引数)を変数として登録する
    fn gen_param_asm(&mut self, node: &inst::Inst) {
        let inst::Inst::Param(param) = node else {
            return;
        };
        let ty = param.ty.clone();
        // 引数に使うレジスタを取得する
        let reg_num = self.arg_register_mode(param.num, &ty);
        self.insert_var_info(
            &param.name,
            asm_emitter::VarIndexInfo::new(reg_num, &ty.try_into().unwrap(), param.dst),
        );
    }

    /// 関数を呼ぶノード(`Inst::CallFunc`)のアセンブリを生成する
    fn gen_call_node_asm(
        &mut self,
        node_idx: usize,
        meta_data: &inst::CallFuncMetaData,
    ) -> Result<(), err::ErrKind> {
        // 関数を呼ぶノードが変数に戻り値を代入しないばあいのみ生成
        if meta_data.parent == ir::IS_NOT_ASSIGN_EXPR {
            let asm_text = self.emit_call_func(meta_data, false)?;
            self.asm_text.push_str(asm_text.as_str());
            // 値として参照された場合(`a: int = func(10)`など)に
            // 戻り値のレジスタを返せるよう、生成済みとして記録する
            self.fn_state.emitted_calls.push(node_idx);
        }
        Ok(())
    }
}
