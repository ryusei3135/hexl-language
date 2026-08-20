use super::*;

impl AsmEmitter {
    pub(super) fn write_mem(
        &mut self,
        name: &String,
        dst: &usize,
        value: &usize,
        this_is_self: bool,
    ) {
        // 書き込み先のメモリのオペランド
        let dst_operand = self.extract_operand_text(dst, this_is_self);
        // `[ptr] = 10`のように、ポインタが指す先のメモリへ直接
        // 書き込む場合は、そのレジスタ/値を`(%rcx)`のように
        // 間接参照するオペランドとして組み立てる。
        // (実際のIRでは`[ptr]`も`Pointer(GetAddress(ptr))`という
        //  形で表現されるため、内側が`GetAddress`かどうかに関わらず
        //  `dst`が`Pointer`である限り常に括弧で囲む必要がある)
        let dst_operand = if matches!(self.curr_inst[*dst], inst::Inst::Pointer(..)) {
            format!("({})", dst_operand)
        } else {
            dst_operand
        };
        // 書き込む値のオペランド
        let value_operand = self.extract_operand_text(value, this_is_self);

        let mut text = self
            .asm_fmt
            .get_opcode_tmpl("mov")
            .replace("{dst}", &dst_operand)
            .replace("{src1}", &value_operand);

        text = self
            .asm_fmt
            .fmt_mnemonic_resize("mov", &text, &self.get_var_ty(&name));
        self.asm_text.push_str(&text);
    }

    pub(super) fn assign_value_ty_is_ptr(
        &mut self,
        current_reg: usize,
        value: &usize,
        this_is_self: bool,
    ) -> String {
        // ポインタ型の変数へ数値リテラル(`ptr = 0`のようなNULL代入)を
        // 再代入する場合は、アドレスを求める`lea`ではなく、ポインタの
        // サイズ(64bit)に合わせた`movq`でそのまま即値を書き込む
        if matches!(self.curr_inst[*value], inst::Inst::Num { .. }) {
            let dst_reg = self.asm_fmt.get_fmt_reg(&current_reg, &Size::DQ);
            let value_operand = self.extract_operand_text(value, this_is_self);
            let text = self
                .asm_fmt
                .get_opcode_tmpl("mov")
                .replace("{dst}", &dst_reg)
                .replace("{src1}", &value_operand);
            return self.asm_fmt.fmt_mnemonic_resize("mov", &text, &Size::DQ);
        }

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

        self.format_line(mnemonic, Some(&current_reg), &value, None, this_is_self)
    }
}
