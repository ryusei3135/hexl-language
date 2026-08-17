use super::*;


impl AsmEmitter {
    pub(super) fn write_mem(
        &mut self, 
        name: &String,
        dst: &usize,
        value: &usize,
        this_is_self: bool
    ) { 
        // 書き込み先のメモリのオペランド
        let dst_operand = self.extract_operand_text(dst, this_is_self);
        // 書き込む値のオペランド
        let value_operand = self.extract_operand_text(value, this_is_self);

        let mut text = self.asm_fmt
            .get_opcode_tmpl("mov")
            .replace("{dst}", &dst_operand)
            .replace("{src1}", &value_operand);

        text = self.asm_fmt.fmt_mnemonic_resize("mov", &text, &self.get_var_ty(&name));
        self.asm_text.push_str(&text);
    }

    pub(super) fn assign_value_ty_is_ptr(
        &mut self,
        current_reg: usize,
        value: &usize,
        this_is_self: bool,
    ) -> String {
        let dst_reg = self.asm_fmt.get_fmt_reg(&current_reg, &Size::DQ);

        let ptr_operand = match &self.curr_inst[*value] {
            inst::Inst::GetPtr { size, stk } => {
                self.asm_fmt.fmt_ref_operand(&"rbp".to_string(), &size)
            }
            _ => self.extract_operand_text(value, this_is_self),
        };

        self.asm_fmt
            .get_opcode_tmpl("address")
            .replace("{dst}", &dst_reg)
            .replace("{src1}", &ptr_operand)
    }

    pub(super) fn assign_value_is_not_ptr(
        &mut self,
        current_reg: usize,
        value: &usize,
        this_is_self: bool,
    ) -> String {
        // 代入する値(value)がアドレスを求める式
        // (`GetAddress`/`Pointer`)の場合のみ`lea`相当の
        // ニーモニックを使う。
        // (代入先の変数がかつてポインタとして定義された
        //  ものであっても、今回代入する値自体が
        //  アドレス計算を必要としないなら`mov`で良い)
        let mnemonic = if self.curr_inst[*value].is_pointer() {
            "address"
        } else {
            "mov"
        };

        self.format_line(
            mnemonic, 
            Some(&current_reg), 
            &value, 
            None,
            this_is_self,
        )
    }
}
