use super::*;

impl AsmEmitter {
    /// 引数`param`を引数レジスタへ渡す命令の種類を決める
    ///
    /// - `"address"`: 値ではなくアドレスを渡す(`lea`)
    /// - `"mov"`: 値をそのまま渡す
    fn arg_opcode(&self, param: usize, index: usize, expand_struct_return: bool) -> &'static str {
        if expand_struct_return && index == 0 {
            "address"
        } else if self.is_ptr_cell_load(param) {
            // `self.ptr`のように、メモリに置かれたポインタの値を渡す
            "mov"
        } else if self.fn_state.curr_inst[param].is_pointer() {
            "address"
        } else if self.fn_state.curr_inst[param]
            .get_param_ty(&self.fn_state.curr_inst)
            .is_some_and(|ty| ty.is_float())
        {
            match self.fn_state.curr_inst[param].get_param_ty(&self.fn_state.curr_inst) {
                Some(types::Size::F32) => "float_mov",
                Some(types::Size::F64) => "f64_mov",
                _ => panic!("floating-point argument has non-floating-point type"),
            }
        } else if matches!(self.fn_state.curr_inst[param], inst::Inst::Str { .. }) {
            // 文字列の実体は静的領域(`M0`など)にあり、引数には
            // その「アドレス」を渡す必要がある。`mov`だとラベルの指す
            // 中身を読み込んでしまうので、`leaq M0(%rip), %rdi`のように
            // `lea`(=`address`)でアドレスを求める
            "address"
        } else {
            "mov"
        }
    }

    /// 引数のノード`idx`が、ポインタ型の関数の引数(`Inst::Param`)で、
    /// 値をそのまま渡す(`mov`)場合に、その変数名を返す
    ///
    /// `extract_operand_text`は、ポインタ型の`Inst::Param`を
    /// 参照先(`-1(%rdi)`など)のオペランドにする(`*p`の読み出し用)。
    /// 呼び出しの引数ではポインタ自身の値を渡したいので、
    /// この場合は`var_operand`でポインタを持つレジスタを直接使う。
    fn ptr_param_name(&self, idx: usize, opcode: &str) -> Option<String> {
        if opcode != "mov" {
            return None;
        }
        let node = &self.fn_state.curr_inst[idx];
        match node {
            inst::Inst::Param(param)
                if node
                    .get_param_ty(&self.fn_state.curr_inst)?
                    .is_pointer()
                    .is_some() =>
            {
                Some(param.name.to_string())
            }
            _ => None,
        }
    }

    /// 各引数を引数レジスタへ設定する命令を作る
    ///
    /// 戻り値は次の2つ。
    /// - 引数レジスタで渡す引数の`(引数レジスタの番号, 値の読み出し元のテキスト, 生成したasm)`。
    ///   並び順は引数の順で、実際の出力順は`order_arg_lines`で決める
    /// - 引数レジスタに収まらない引数(`param_in_reg`が`false`)を、スタックへ積む命令
    ///   (引数の順。`emit_call_func`が最後の引数から順に出力する。`push_arg_asm`)
    pub(super) fn build_arg_lines(
        &mut self,
        meta_data: &inst::CallFuncMetaData,
        expand_struct_return: bool,
    ) -> Result<(Vec<(RegisterMode, String, String)>, Vec<String>), err::ErrKind> {
        let mut arg_lines: Vec<(RegisterMode, String, String)> = Vec::new();
        let mut stack_args: Vec<String> = Vec::new();

        for (index, param) in meta_data.params.iter().enumerate() {
            // 引数の実際の型のサイズ
            // (レジスタの取得だけでなく、後段のニーモニックの
            //  サイズ決定にも同じサイズを使う必要がある)
            let param_ty = self.fn_state.curr_inst[*param]
                .get_param_ty(&self.fn_state.curr_inst)
                .ok_or_else(|| err::ErrKind::asm_emit_label_not_found("param type", *param))?;
            // 引数のレジスタを取得(レジスタに収まらない引数は`None`)
            let param_reg = if self.param_in_reg(index) {
                Some(self.asm_fmt.get_fmt_param::<String>(index, &param_ty)?)
            } else {
                None
            };
            // 引数のレジスタと値のidを挿入
            let src1_idx = if let Some(struct_idx) = self.resolve_struct_idx(*param) {
                struct_idx
            } else {
                *param
            };
            if param_reg.is_some() {
                let param_reg_mode = self.arg_register_mode(index, &param_ty);
                self.used_reg.mark_used(&param_reg_mode);
            }

            let opcode = self.arg_opcode(*param, index, expand_struct_return);

            let resize_size = if opcode == "address" || param_ty.is_pointer().is_some() {
                Size::DQ
            } else {
                param_ty.clone()
            };

            // 呼び出し側の引数(ポインタ)をそのまま渡す場合は、
            // 参照先(`-1(%rdi)`)ではなく、ポインタを持つレジスタを移動する
            let (src1_text, asm) = if let Some(name) = self.ptr_param_name(src1_idx, opcode) {
                let src_text = self.var_operand(&name)?;
                let is_stack = self.get_var_all_info(&name)?.is_stack;
                let asm = match &param_reg {
                    // 同じレジスタへの`mov`(`movq %rdi, %rdi`)は何もしない
                    Some(param_reg) if src_text == *param_reg => String::new(),
                    Some(param_reg) => {
                        self.tmpl_line("mov", param_reg, &src_text, &Size::DQ, is_stack)
                    }
                    None => {
                        stack_args.push(self.push_arg_asm(
                            opcode,
                            &src_text,
                            &Size::DQ,
                            is_stack,
                        )?);
                        String::new()
                    }
                };
                (src_text, asm)
            } else {
                let src1_text =
                    self.extract_operand_text(src1_idx, resize_size.wrap_dst_size().as_ref())?;
                let src1_text = self.asm_fmt.resize_reg_operand(&src1_text, &resize_size)?;

                let is_memory = self.check_node_is_mem_val(*param).is_some();
                let asm = match &param_reg {
                    Some(param_reg) => {
                        self.tmpl_line(opcode, param_reg, &src1_text, &resize_size, is_memory)
                    }
                    None => {
                        stack_args.push(self.push_arg_asm(
                            opcode,
                            &src1_text,
                            &resize_size,
                            is_memory,
                        )?);
                        String::new()
                    }
                };
                (src1_text, asm)
            };
            if param_reg.is_some() {
                let param_reg_num = self.asm_fmt.get_fmt_param::<usize>(index, &param_ty)?;
                let param_reg_num = if param_ty.is_float() {
                    RegisterMode::Float(param_reg_num)
                } else {
                    RegisterMode::Normal(param_reg_num)
                };
                arg_lines.push((param_reg_num, src1_text, asm));
            }
        }
        Ok((arg_lines, stack_args))
    }

    /// `src`の値(引数1つ分)を、8byteの領域としてスタックへ積む命令を作る
    ///
    /// 呼ばれた側は、サイズに合わせて`16(%rbp)`などから読むので、積んだ値の
    /// 上位のビットは使われない。`opcode`は`arg_opcode`が決めた命令の種類
    ///
    /// - 整数・ポインタ: `%rax`(`RESERVED_REGS`なので、変数や式の結果には使われていない)へ
    ///   読み込んで`push`する。64bitのレジスタなら直接`push`する。
    ///   `"address"`のときは、アドレスを求めて(`lea`)から`push`する
    /// - 浮動小数点: `xmm`のレジスタなら、領域を確保して`mov`で書き込む。
    ///   メモリや定数(`M0(%rip)`)は、整数のレジスタ経由でビットをそのまま積む
    fn push_arg_asm(
        &mut self,
        opcode: &str,
        src: &str,
        size: &Size,
        is_memory: bool,
    ) -> Result<String, err::ErrKind> {
        let src_is_reg = self
            .asm_fmt
            .all_reg_names()?
            .iter()
            .any(|(_, name)| name == src);

        // `xmm`のレジスタの浮動小数点数
        if size.is_float() && src_is_reg {
            let top = self.asm_fmt.get_stack_top_ref();
            let store = self.fill_tmpl(Self::float_move_opcode(size), &top, src);
            return Ok(format!("{}{}", self.asm_fmt.get_stack_alloc_8(), store));
        }

        // ここからは、整数のレジスタ(%rax)を経由して積む
        let scratch = RegisterMode::Normal(0);
        let scratch64 = self.reg64(&scratch)?;
        let load = if opcode == "address" {
            self.tmpl_line("address", &scratch64, src, &Size::DQ, is_memory)
        } else {
            // 浮動小数点数のメモリは、同じ幅の整数としてビットをコピーする
            let int_size = match size {
                Size::F32 => Size::DD,
                Size::F64 => Size::DQ,
                _ => Self::value_reg_size(size),
            };
            if src_is_reg {
                // 整数のレジスタは、同じレジスタの64bit版を直接`push`する
                let src64 = self.asm_fmt.resize_reg_operand(src, &Size::DQ)?;
                return Ok(self.asm_fmt.get_push(&src64));
            }
            let dst = self.reg_mode_operand(scratch, &int_size)?;
            self.mov_line(&dst, src, &int_size, true)
        };
        Ok(format!("{}{}", load, self.asm_fmt.get_push(&scratch64)))
    }

    /// 引数レジスタへ値を設定する命令を、レジスタを壊さない順序に並べて返す
    ///
    /// 先に`%rdi`へ値を書いてしまうと、後ろの引数が元の`%rdi`の値を
    /// 読む場合(`f(y, x)`で、`x`が`%rdi`にある場合など)に、
    /// 壊れた値を読んでしまう。「他の未出力の引数から読まれていない
    /// 引数レジスタ」を持つ引数から順に出力する。
    /// (引数同士が循環している場合だけは、元の順のまま出力する)
    pub(super) fn order_arg_lines(
        &self,
        arg_lines: Vec<(RegisterMode, String, String)>,
    ) -> Result<String, err::ErrKind> {
        let mut asm = String::new();
        let mut pending: Vec<usize> = (0..arg_lines.len()).collect();

        while !pending.is_empty() {
            let mut found: Option<usize> = None;
            for (p, i) in pending.iter().enumerate() {
                let mut readable = false;
                for j in 0..arg_lines.len() {
                    if j != *i && self.text_reads_reg(&arg_lines[j].1, arg_lines[*i].0)? {
                        readable = true;
                        break;
                    }
                }
                if !readable {
                    found = Some(p);
                    break;
                }
            }
            let pos = found.unwrap_or(0);
            let i = pending.remove(pos);
            asm.push_str(&arg_lines[i].2);
        }
        Ok(asm)
    }

    /// オペランドのテキストが、レジスタ`reg`(どのサイズの名前でも)を
    /// 読み出しに使っているか(`(%rbx)`のようなメモリ参照も含む)
    pub(super) fn text_reads_reg(
        &self,
        text: &str,
        reg: RegisterMode,
    ) -> Result<bool, err::ErrKind> {
        Ok(self
            .asm_fmt
            .all_reg_names()?
            .iter()
            .any(|(num, name)| *num == reg && text.contains(name.as_str())))
    }
}
