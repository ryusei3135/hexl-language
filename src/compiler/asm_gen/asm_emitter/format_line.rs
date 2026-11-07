//! IRのノード1つぶんを、フォーマットのテンプレートで1行のアセンブリにする処理

use super::*;

impl AsmEmitter {
    /// 渡された情報を、設定したアセンブリ言語のフォーマット
    /// 通りに加工する。
    pub(in crate::compiler::asm_gen) fn format_line(
        &mut self,
        opcode: &str,
        dst: Option<usize>,
        src1: usize,
        src2: Option<usize>,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let mut formated = if let Some(variant_idx) = self.resolve_variant_idx(src1) {
            self.format_variant_init_line(variant_idx, dst, this_is_self)?
        } else if let Some(struct_idx) = self.resolve_struct_idx(src1) {
            self.format_struct_init_line(opcode, struct_idx, dst, src1, this_is_self)?
        } else {
            self.format_value_line(opcode, dst, src1, this_is_self)?
        };

        if let Some(resized_asm) = self.gen_resize_mnemonic(&formated, opcode, src1) {
            formated = resized_asm;
        }

        Ok(if let Some(src2_id) = src2 {
            formated.replace("{src2}", &self.extract_operand_text(src2_id, this_is_self)?)
        } else {
            formated
        })
    }

    /// バリアントの生成: 初期化の命令を出した後、先頭(タグ)の
    /// アドレスをdstへ渡す(構造体と同じ扱い)
    fn format_variant_init_line(
        &mut self,
        variant_idx: usize,
        dst: Option<usize>,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let mut txt = self.extract_operand_text(variant_idx, this_is_self)?;
        let is_self = matches!(
            self.fn_state.curr_inst[variant_idx],
            inst::Inst::Variant { is_self: true, .. }
        );
        if !is_self {
            let stk = self.ptr_stk(variant_idx, 0);
            let src = self.rbp_ref(stk);
            let dst_text = self.get_reg_txt(dst, &Size::DQ).unwrap();
            let line = self.fill_tmpl("address", &dst_text, &src);
            txt.push_str(&self.asm_fmt.fmt_mnemonic_resize("lea", &line, &Size::DQ));
        }
        Ok(txt)
    }

    /// 構造体の生成: 初期化の命令を出した後、構造体のアドレス
    /// (`self`の中なら暗黙の`self`ポインタ)をdstへ渡す
    fn format_struct_init_line(
        &mut self,
        opcode: &str,
        struct_idx: usize,
        dst: Option<usize>,
        src1: usize,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let base_size: Size = self.check_node_is_mem_val(src1).unwrap_or(Size::DQ);
        let mut txt = self.extract_operand_text(struct_idx, this_is_self)?;

        let ret_line = if this_is_self.is_none() {
            if let Some(offset) = self.fn_state.self_ptr_slot {
                // 退避したポインタの値をそのままdstへ渡す
                self.mov_line(
                    &self.get_reg_txt(dst, &Size::DQ)?,
                    &self.rbp_ref(offset),
                    &Size::DQ,
                    true,
                )
            } else {
                let self_ptr_reg = self.self_ptr_reg();
                // `self`のポインタはレジスタに入っているので、`fill_tmpl`が`address`を
                // `mov`に置き換える(`leaq %rdi, ..`は不正な命令)。
                // どちらでも、ポインタを渡すので64bitで扱う
                let line =
                    self.fill_tmpl("address", &self.get_reg_txt(dst, &Size::DQ)?, &self_ptr_reg);
                let line = self.asm_fmt.fmt_mnemonic_resize("lea", &line, &Size::DQ);
                self.asm_fmt.fmt_mnemonic_resize("mov", &line, &Size::DQ)
            }
        } else {
            self.fill_tmpl(opcode, &self.get_reg_txt(dst, &base_size)?, FRAME_BASE_REG)
        };
        txt.push_str(&ret_line);
        Ok(txt)
    }

    /// 普通の値(数値・変数・式の結果など)をdstへ渡す命令を作る
    fn format_value_line(
        &mut self,
        opcode: &str,
        dst: Option<usize>,
        src1: usize,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let dst_size = self.resolve_dst_reg_size(opcode, src1, this_is_self);
        let dst_text = self.get_reg_txt(dst, &dst_size)?;
        let src_text = self.extract_operand_text(src1, this_is_self)?;
        // srcがレジスタの場合は、dstとサイズを揃える
        // (`movl %rcx, %edx`のような、サイズの混在を防ぐ)
        let src_text = if opcode == "address" {
            src_text
        } else {
            self.asm_fmt.resize_reg_operand(&src_text, &dst_size)?
        };
        Ok(self.fill_tmpl(opcode, &dst_text, &src_text))
    }

    #[inline(always)]
    fn gen_resize_mnemonic(&self, formated: &str, opcode: &str, src1: usize) -> Option<String> {
        let mut resize = self.check_node_is_mem_val(src1).unwrap_or(Size::DQ);
        let mnemonic: &str = if opcode == "address" {
            resize = Size::DQ;
            "lea"
        } else if self.check_node_is_struct(src1) {
            "mov"
        } else if let Some(size) = self.check_node_is_mem_val(src1) {
            resize = size;
            "mov"
        } else {
            return None;
        };
        let resized = if self.check_node_is_mem_val(src1).is_some() {
            self.asm_fmt
                .fmt_memory_mnemonic_resize(mnemonic, &formated, &resize)
        } else {
            self.asm_fmt
                .fmt_mnemonic_resize(mnemonic, formated, &resize)
        };
        Some(resized)
    }
}
