use super::*;

impl AsmEmitter {
    /// 配列の添字が構造体のメンバー変数の場合に、構造体から添字を取り出して
    /// 添字付きアドレッシングのオペランドを生成する
    ///
    /// `[arr a.a else 0] = 0`(`arr: [int 5]`)なら、次のように出力する:
    /// ```gas
    /// movl -28(%rbp), %ecx          # 添字を取り出す
    /// movslq %ecx, %rcx             # 64bitへ符号拡張(アドレス計算は64bitで行う)
    /// negq %rcx                     # 要素はアドレスが小さくなる向きに並ぶため
    /// addq $4, %rcx                 # 添字を反転する(要素数 - 1 - 添字)
    /// movl $0, -20(%rbp, %rcx, 4)   # 戻り値のオペランド(4は要素のサイズ)
    /// ```
    ///
    /// 配列の要素`i`は`%rbp - (arr_base + (i + 1) * 要素サイズ)`に置かれる
    /// (`mem_val_ir`・`init_arr_txt`と同じ配置)。x86のスケールは正の値しか
    /// 取れないので、添字を`要素数 - 1 - 添字`に変換し、最後の要素の位置
    /// `-(arr_base + 要素数 * 要素サイズ)(%rbp, 変換後の添字, 要素サイズ)`を
    /// 基準に参照する。
    ///
    /// 添字を載せた一時レジスタは`self.arr_index_temp`に記録し、
    /// 書き込み後に`write_mem`が解放する
    pub(in crate::asm_gen::asm_emitter)
    fn ref_arr_for_struct_member(
        &mut self,
        name: &str,
        src: &str,
        pos: usize,
        size: &Size,
        dst: usize,
        this_is_self: &SelfPtrInfo,
    ) -> String {
        // 前回の添字用レジスタが解放されていなければ、ここで解放する
        if let Some(old) = self.arr_index_temp.take() {
            self.used_reg.release(old);
        }

        // 配列(またはポインタ)の情報と、ベースになるレジスタ
        // - ローカル配列: ベースは`%rbp`(`dst`のオペランドは`-4(%rbp)`のような
        //   メモリ参照になるため使わない。使うと`-4(-4(%rbp), ..)`になる)
        // - ポインタ: ポインタ自身を持つ64bitレジスタがベース
        let (elem_size, arr_base, is_ptr, base, arr_len) = {
            let var_info = self
                .var_hash_map
                .get(name)
                .unwrap_or_else(|| panic!("array variable not found: {}", name));
            match &var_info.size {
                Size::Array { size, len } => (
                    size.to_bytes(),
                    var_info.arr_base,
                    false,
                    "%rbp".to_string(),
                    *len,
                ),
                Size::Pointer { ty, .. } => (
                    ty.to_bytes(),
                    0,
                    true,
                    self.asm_fmt.get_fmt_reg(var_info.reg, &Size::DQ),
                    0,
                ),
                other => (
                    other.to_bytes(),
                    var_info.arr_base,
                    false,
                    "%rbp".to_string(),
                    1,
                ),
            }
        };
        let _ = (dst, this_is_self);

        // 構造体のメンバーを、添字用のレジスタへ読み込む
        let member = self.ref_struct_txt(src, pos, size.to_bytes());
        let reg_num = self.alloc_reg();
        self.used_reg.mark_used(reg_num);
        self.arr_index_temp = Some(reg_num);

        let idx_size = Self::value_reg_size(size);
        let reg64 = self.asm_fmt.get_fmt_reg(reg_num, &Size::DQ);

        match idx_size {
            // 8/16bitの添字は、メモリから直接符号拡張して読み込む
            Size::DB | Size::DW => {
                let kind = if matches!(idx_size, Size::DB) { 'b' } else { 'w' };
                let load = self.asm_fmt.get_sign_extend(kind, &member, &reg64);
                self.asm_text.push_str(&load);
            }
            // 32bitの添字: 32bitで読み込んでから64bitへ符号拡張する
            //   movl -28(%rbp), %ecx
            //   movslq %ecx, %rcx
            Size::DD => {
                let reg32 = self.asm_fmt.get_fmt_reg(reg_num, &Size::DD);
                let load = self
                    .asm_fmt
                    .get_opcode_tmpl("mov")
                    .replace("{dst}", &reg32)
                    .replace("{src1}", &member);
                let load = self
                    .asm_fmt
                    .fmt_memory_mnemonic_resize("mov", &load, &Size::DD);
                self.asm_text.push_str(&load);
                let ext = self.asm_fmt.get_sign_extend('l', &reg32, &reg64);
                self.asm_text.push_str(&ext);
            }
            // 64bitの添字: そのまま読み込む
            _ => {
                let load = self
                    .asm_fmt
                    .get_opcode_tmpl("mov")
                    .replace("{dst}", &reg64)
                    .replace("{src1}", &member);
                let load = self
                    .asm_fmt
                    .fmt_memory_mnemonic_resize("mov", &load, &Size::DQ);
                self.asm_text.push_str(&load);
            }
        }

        // 64bitへ符号拡張した添字の符号を反転する
        // (要素はアドレスが小さくなる向きに並んでいる)
        let neg = self.asm_fmt.get_neg(&reg64);
        let neg = self.asm_fmt.fmt_mnemonic_resize("neg", &neg, &Size::DQ);
        self.asm_text.push_str(&neg);

        // 配列は添字を `要素数 - 1 - 添字` に反転して、最後の要素
        // (最も%rbpから遠い要素)の位置を基準に参照する
        // (ポインタは要素0を指しているのでオフセットなし)
        let offset = if is_ptr {
            0
        } else {
            if arr_len > 1 {
                let add = self
                    .asm_fmt
                    .get_add_imm(&self.asm_fmt.get_fmt_num(&(arr_len - 1).to_string()), &reg64);
                let add = self.asm_fmt.fmt_mnemonic_resize("add", &add, &Size::DQ);
                self.asm_text.push_str(&add);
            }
            arr_base + elem_size * arr_len.max(1)
        };
        self.asm_fmt
            .fmt_ref_operand_indexed(&base, &reg64, elem_size, offset)
    }
}
