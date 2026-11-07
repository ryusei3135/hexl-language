use super::*;

/// 命令のオペランドの種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OperandKind {
    Register,
    Immediate,
    Memory,
}

impl AsmEmitter {
    /// オペランドのテキストが、レジスタ・即値・メモリのどれか
    fn operand_kind(&self, operand: &str) -> Result<OperandKind, err::ErrKind> {
        if self
            .asm_fmt
            .all_reg_names()?
            .iter()
            .any(|(_, name)| name == operand)
        {
            return Ok(OperandKind::Register);
        }
        let num_prefix = self.asm_fmt.get_fmt_num("0");
        let num_prefix = num_prefix.trim_end_matches('0');
        if operand.parse::<i128>().is_ok()
            || (!num_prefix.is_empty() && operand.starts_with(num_prefix))
        {
            return Ok(OperandKind::Immediate);
        }
        Ok(OperandKind::Memory)
    }

    /// メモリからメモリへ値を書くときに、値を経由させる一時レジスタ
    ///
    /// `%rax`/`%rdx`(`RESERVED_REGS`なので変数や式の結果には使われていない)のうち、
    /// 書き込み先のオペランド`dst_operand`が使っていないほうを返す
    fn mem_copy_scratch(&self, dst_operand: &str) -> Result<RegisterMode, err::ErrKind> {
        for reg in RESERVED_REGS {
            let reg = RegisterMode::Normal(reg);
            if !self.text_reads_reg(dst_operand, reg)? {
                return Ok(reg);
            }
        }
        Ok(RegisterMode::Normal(RESERVED_REGS[0]))
    }

    /// メモリに値を書き込むアセンブリ言語を生成
    pub(super) fn write_mem(
        &mut self,
        name: &str,
        dst: usize,
        value: usize,
        this_is_self: SelfPtrInfo,
    ) -> Result<(), err::ErrKind> {
        // 書き込み先のメモリのオペランド
        let dst_operand = self.extract_operand_text(dst, this_is_self)?;
        let dst_operand = if matches!(self.fn_state.curr_inst[dst], inst::Inst::Pointer(..)) {
            format!("({})", dst_operand)
        } else {
            dst_operand
        };
        // 書き込む値のオペランド
        let value_operand = self.extract_operand_text(value, this_is_self)?;

        // ニーモニックのサイズ調整に使う型。
        //
        // `[ptr] = 20`/`[arr 0] = 10`のケースでは、`name`(ポインタ/
        // 配列の変数名)の型(`Pointer{ty}`/`Array{size,..}`)がそのまま
        // 書き込む値のサイズを表しているため、`get_var_ty(&name)`で
        // 問題なかった。
        let mnemonic_size = match &self.fn_state.curr_inst[dst] {
            // 構造体のメンバー(`self->len = ..` / `a.len = ..`)は、メンバー自身の型。
            // `name`(`self`など)の型は構造体(またはそのポインタ)なので、
            // ニーモニックのサイズに使えない
            inst::Inst::RefStruct { size, .. } | inst::Inst::RefStructPtr { size, .. } => {
                size.clone()
            }
            inst::Inst::Pointer(..) | inst::Inst::InsertArr { .. } => self.get_expr_ty(dst),
            _ => self.get_var_ty(&name)?,
        };

        let value_kind = self.operand_kind(&value_operand)?;
        let text = if mnemonic_size.is_float() && value_kind != OperandKind::Memory {
            let opcode = match mnemonic_size {
                types::Size::F32 => "float_mov",
                types::Size::F64 => "f64_mov",
                _ => unreachable!(),
            };
            self.asm_fmt
                .get_opcode_tmpl(opcode)
                .replace("{src1}", &value_operand)
                .replace("{dst}", &dst_operand)
        } else {
            // 浮動小数点数のメモリからメモリへは、同じ幅の整数としてビットをコピーする
            let store_size = match mnemonic_size {
                types::Size::F32 => Size::DD,
                types::Size::F64 => Size::DQ,
                _ => Self::value_reg_size(&mnemonic_size),
            };
            match value_kind {
                // レジスタは、書き込むサイズの名前にする
                // (`movb %r8`や`movl %rax`は、サイズが食い違う不正な命令)
                OperandKind::Register => {
                    let value = self
                        .asm_fmt
                        .resize_reg_operand(&value_operand, &store_size)?;
                    self.mov_line(&dst_operand, &value, &mnemonic_size, true)
                }
                OperandKind::Immediate => {
                    self.mov_line(&dst_operand, &value_operand, &mnemonic_size, true)
                }
                // メモリからメモリへは`mov`できないので、一時レジスタを経由する
                OperandKind::Memory => {
                    let scratch = self.mem_copy_scratch(&dst_operand)?;
                    let scratch_txt = self.reg_mode_operand(scratch, &store_size)?;
                    let load = self.mov_line(&scratch_txt, &value_operand, &store_size, true);
                    let store = self.mov_line(&dst_operand, &scratch_txt, &store_size, true);
                    format!("{load}{store}")
                }
            }
        };
        self.asm_text.push_str(&text);
        // 配列の添字を載せていた一時レジスタは、書き込みが終わったので解放する
        if let Some(reg) = self.fn_state.arr_index_temp.take() {
            self.used_reg.release(&RegisterMode::Normal(reg));
        }
        Ok(())
    }

    pub(super) fn assign_val_ty_is_ptr(
        &mut self,
        current_reg: RegisterMode,
        value: usize,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let RegisterMode::Normal(_) = current_reg else {
            panic!("pointer variable is not stored in a normal register");
        };
        // ポインタ型の変数へ数値リテラル(`ptr = 0`のようなNULL代入)を
        // 再代入する場合は、アドレスを求める`lea`ではなく、ポインタの
        // サイズ(64bit)に合わせた`movq`でそのまま即値を書き込む
        if matches!(self.fn_state.curr_inst[value], inst::Inst::Num { .. }) {
            let dst_reg = self.reg64(&current_reg)?;
            let value_operand = self.extract_operand_text(value, this_is_self)?;
            return Ok(self.mov_line(&dst_reg, &value_operand, &Size::DQ, false));
        }

        let dst_reg = self.reg64(&current_reg)?;

        // `value`が実際に「メモリ上の場所」を指している場合のみ、その
        // アドレスを`lea`で求める必要がある。`value`が既にアドレス値
        // そのもの(ポインタ演算の結果やレジスタに載ったポインタ変数
        // など)を持っている場合は、それをそのまま`mov`でコピーすれば
        // よく、`lea`をかけると`lea %rdx, %rbx`のような、メモリでは
        // ないソースを持つ不正な命令になってしまう。
        let (ptr_operand, needs_address) = match &self.fn_state.curr_inst[value] {
            inst::Inst::GetPtr { stk, .. } => (self.rbp_ref(self.ptr_stk(value, *stk)), true),
            _ => {
                let operand = self.extract_operand_text(value, this_is_self)?;
                let needs_address = self.check_node_is_mem_val(value).is_some();
                (operand, needs_address)
            }
        };

        let opcode = if needs_address { "address" } else { "mov" };

        Ok(self.tmpl_line(opcode, &dst_reg, &ptr_operand, &Size::DQ, false))
    }

    pub(super) fn assign_val_is_not_ptr(
        &mut self,
        current_reg: RegisterMode,
        value: usize,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let RegisterMode::Normal(current_reg) = current_reg else {
            panic!("non-floating-point value assigned to a floating-point register");
        };
        let mnemonic =
            if self.fn_state.curr_inst[value].is_pointer() && !self.is_ptr_cell_load(value) {
                "address"
            } else {
                "mov"
            };
        self.format_line(mnemonic, Some(current_reg), value, None, this_is_self)
    }

    pub(super) fn assign_val_is_float(
        &mut self,
        current_reg: RegisterMode,
        value_idx: usize,
        size: &types::Size,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let RegisterMode::Float(index) = current_reg else {
            panic!("floating-point value assigned to a normal register");
        };
        let opcode = match size {
            types::Size::F32 => "float_mov",
            types::Size::F64 => "f64_mov",
            _ => panic!(
                "floating-point variable has non-floating-point type: {:?}",
                size
            ),
        };
        let dst = self.asm_fmt.get_fmt_float_reg(index, size)?;
        let src = self.extract_operand_text(value_idx, this_is_self)?;
        Ok(self
            .asm_fmt
            .get_opcode_tmpl(opcode)
            .replace("{src1}", &src)
            .replace("{dst}", &dst))
    }
}
