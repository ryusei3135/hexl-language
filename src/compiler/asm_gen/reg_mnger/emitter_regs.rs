//! `AsmEmitter`が式の結果・変数を置くレジスタを確保する処理

use super::*;

impl AsmEmitter {
    /// 式の結果や変数を置くレジスタを確保して返す。
    ///
    /// 以前は`self.reg_idx`を単純にインクリメントしていたため、
    /// 引数が入っているレジスタ(`%rdi`/`%rsi`など)や、他の変数が
    /// 使っているレジスタまで上書きしていた。ここでは
    /// - 使用中(引数・変数・まだ使う式の結果)のレジスタ
    /// - `RESERVED_REGS`(`%rax`/`%rdx`)
    /// を避けて最も小さい番号の空きレジスタを返す。空きが無い場合は
    /// 生きている値をスタックへ退避して、そのレジスタを再利用する。
    ///
    /// 返したレジスタを使用中にするのは呼び出し側の責任
    /// (`mark_used`を呼ぶ)。
    pub(in crate::compiler::asm_gen) fn alloc_reg(
        &mut self,
        size: &Size,
    ) -> Result<Option<RegisterMode>, err::ErrKind> {
        if let Some(reg) = self.try_alloc_reg(size) {
            return Ok(Some(reg));
        }

        let wants_float = size.is_float();
        let candidates = self.used_reg.used_regs();
        for reg in candidates {
            if matches!(reg, RegisterMode::Float(_)) != wants_float {
                continue;
            }
            if self.spill_reg_to_stack(reg)? {
                if let Some(free_reg) = self.try_alloc_reg(size) {
                    return Ok(Some(free_reg));
                }
            }
        }

        if self.used_reg.release_pending_reg(wants_float) {
            return Ok(self.try_alloc_reg(size));
        }
        Ok(None)
    }

    /// [`alloc_reg`](Self::alloc_reg)と同じ条件で空きレジスタを探し、
    /// 1つも無ければ`None`を返す(panicしない)。
    ///
    /// 変数を定義するときに、`None`ならスタックへ置く
    /// (`define_spilled_var`)ために使う。
    pub(in crate::compiler::asm_gen) fn try_alloc_reg(&self, size: &Size) -> Option<RegisterMode> {
        let (count, mode) = if size.is_float() {
            (self.asm_fmt.float_reg_count(), RegisterMode::Float(0))
        } else {
            (self.asm_fmt.reg_count(), RegisterMode::Normal(0))
        };
        (0..count)
            .map(|index| match mode {
                RegisterMode::Normal(_) => RegisterMode::Normal(index),
                RegisterMode::Float(_) => RegisterMode::Float(index),
            })
            .find(|reg| {
                !matches!(reg, RegisterMode::Normal(index) if RESERVED_REGS.contains(index))
                    && !self.used_reg.is_used(reg)
                    // まだ定義されていない変数のために割り当てたレジスタは使わない
                    && !self.used_reg.is_reserved(*reg)
            })
    }

    /// ## 引数
    /// - reg_idx これは必ずusizeで無ければいけない、
    pub(in crate::compiler::asm_gen) fn get_reg_txt(
        &self,
        reg_idx: Option<usize>,
        size: &Size,
    ) -> Result<String, err::ErrKind> {
        let num = if reg_idx.is_none() {
            self.fn_state.reg_idx
        } else {
            // これは必ずSome
            reg_idx.unwrap()
        };
        if size.is_float() {
            self.asm_fmt.get_fmt_float_reg(num, size)
        } else {
            self.asm_fmt.get_fmt_reg(num, size)
        }
    }

    /// `format_line`で、結果を書き込む先(`dst`)のレジスタの
    /// サイズを決める。
    ///
    /// 1. `address`(`lea`)は常にアドレスなので64bit
    /// 2. メモリ上の値を読む場合は、読む対象のサイズ
    /// 3. 即値・式の結果・関数の戻り値など(メモリ以外)は、
    ///    代入先の型(`dst_ty`)のサイズ
    /// 4. `dst_ty`が無い(`self`を持つメソッド内など)場合は、
    ///    値そのものの型から推測できるときだけその型、
    ///    できなければ64bit
    ///
    /// 以前は3.と4.が無く、メモリ以外の値は常に64bitレジスタ
    /// (`%rcx`など)に書き込まれていた。
    pub(in crate::compiler::asm_gen) fn resolve_dst_reg_size(
        &self,
        opcode: &str,
        src1: usize,
        dst_ty: SelfPtrInfo,
    ) -> Size {
        if opcode == "address" {
            return Size::DQ;
        }
        if let Some(size) = self.check_node_is_mem_val(src1) {
            return Self::value_reg_size(&size);
        }
        if let Some(ty) = dst_ty {
            return Self::value_reg_size(ty);
        }
        match &self.fn_state.curr_inst[src1] {
            inst::Inst::Num { size, .. } => Self::value_reg_size(size),
            inst::Inst::Expr(..) => Self::value_reg_size(&self.get_expr_ty(src1)),
            inst::Inst::Cast { after, .. } => Self::value_reg_size(after),
            _ => Size::DQ,
        }
    }
}
