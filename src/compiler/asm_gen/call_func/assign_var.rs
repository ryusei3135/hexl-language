use super::*;

impl AsmEmitter {
    /// メモリに値を書き込むアセンブリ言語を生成
    pub(super) fn write_mem(
        &mut self,
        name: &str,
        dst: usize,
        value: usize,
        this_is_self: SelfPtrInfo,
    ) {
        // 書き込み先のメモリのオペランド
        let dst_operand = self.extract_operand_text(dst, this_is_self);
        let dst_operand = if matches!(self.curr_inst[dst], inst::Inst::Pointer(..)) {
            format!("({})", dst_operand)
        } else {
            dst_operand
        };
        // 書き込む値のオペランド
        let value_operand = self.extract_operand_text(value, this_is_self);

        // ニーモニックのサイズ調整に使う型。
        //
        // `[ptr] = 20`/`[arr 0] = 10`のケースでは、`name`(ポインタ/
        // 配列の変数名)の型(`Pointer{ty}`/`Array{size,..}`)がそのまま
        // 書き込む値のサイズを表しているため、`get_var_ty(&name)`で
        // 問題なかった。
        let mnemonic_size = match &self.curr_inst[dst] {
            inst::Inst::RefStruct { .. } => match &self.curr_inst[value] {
                inst::Inst::Num { size, .. } => size.clone(),
                _ => self.get_var_ty(&name),
            },
            inst::Inst::Pointer(..) | inst::Inst::InsertArr { .. } => self.get_expr_ty(dst),
            _ => self.get_var_ty(&name),
        };

        let text = self.mov_line(&dst_operand, &value_operand, &mnemonic_size, true);
        self.asm_text.push_str(&text);
        // 配列の添字を載せていた一時レジスタは、書き込みが終わったので解放する
        if let Some(reg) = self.arr_index_temp.take() {
            self.used_reg.release(reg);
        }
    }

    pub(super) 
    fn assign_val_ty_is_ptr(
        &mut self,
        current_reg: usize,
        value: usize,
        this_is_self: SelfPtrInfo,
    ) -> String {
        // ポインタ型の変数へ数値リテラル(`ptr = 0`のようなNULL代入)を
        // 再代入する場合は、アドレスを求める`lea`ではなく、ポインタの
        // サイズ(64bit)に合わせた`movq`でそのまま即値を書き込む
        if matches!(self.curr_inst[value], inst::Inst::Num { .. }) {
            let dst_reg = self.reg64(current_reg);
            let value_operand = self.extract_operand_text(value, this_is_self);
            return self.mov_line(&dst_reg, &value_operand, &Size::DQ, false);
        }

        let dst_reg = self.reg64(current_reg);

        // `value`が実際に「メモリ上の場所」を指している場合のみ、その
        // アドレスを`lea`で求める必要がある。`value`が既にアドレス値
        // そのもの(ポインタ演算の結果やレジスタに載ったポインタ変数
        // など)を持っている場合は、それをそのまま`mov`でコピーすれば
        // よく、`lea`をかけると`lea %rdx, %rbx`のような、メモリでは
        // ないソースを持つ不正な命令になってしまう。
        let (ptr_operand, needs_address) = match &self.curr_inst[value] {
            inst::Inst::GetPtr { stk, .. } => (self.rbp_ref(self.ptr_stk(value, *stk)), true),
            _ => {
                let operand = self.extract_operand_text(value, this_is_self);
                let needs_address = self.check_node_is_mem_val(value).is_some();
                (operand, needs_address)
            }
        };

        let opcode = if needs_address { "address" } else { "mov" };

        self.tmpl_line(opcode, &dst_reg, &ptr_operand, &Size::DQ, false)
    }

    pub(super) 
    fn assign_val_is_not_ptr(
        &mut self,
        current_reg: usize,
        value: usize,
        this_is_self: SelfPtrInfo,
    ) -> String {
        let mnemonic = if self.curr_inst[value].is_pointer() {
            "address"
        } else {
            "mov"
        };

        self.format_line(mnemonic, Some(current_reg), value, None, this_is_self)
    }
}