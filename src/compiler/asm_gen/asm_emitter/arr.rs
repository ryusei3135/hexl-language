use super::*;

/// 添字付きアドレッシングの基準になる、配列(またはポインタ)の情報
struct ArrRefBase {
    /// 要素1つのバイト数
    elem_size: usize,
    /// 要素0の位置(`%rbp`からのオフセット。ポインタは0)
    arr_base: usize,
    /// ポインタか(配列ならfalse)
    is_ptr: bool,
    /// ベースになるレジスタ名
    base: String,
    /// 配列の要素数(ポインタは0)
    arr_len: usize,
}

impl AsmEmitter {
    /// 配列の添字が構造体のメンバー変数の場合に、構造体から添字を取り出して
    /// 添字付きアドレッシングのオペランドを生成する
    /// (`[arr a.a else 0]`)
    ///
    /// 添字を載せた一時レジスタは`self.arr_index_temp`に記録し、
    /// 書き込み後に`write_mem`が解放する
    pub(in crate::compiler::asm_gen::asm_emitter)
    fn ref_arr_for_struct_member(
        &mut self,
        name: &str,
        src: &str,
        pos: usize,
        size: &Size,
        dst: usize,
        this_is_self: SelfPtrInfo,
    ) -> String {
        let _ = (dst, this_is_self);
        // 構造体のメンバーの位置(`-4(%rbp)`など)が、添字の読み込み元になる
        let member = self.ref_struct_txt(src, pos, size.to_bytes());
        self.ref_arr_indexed(name, &member, size)
    }

    /// 配列の添字が普通の変数(ローカル変数/関数の引数)の場合に、
    /// 添字付きアドレッシングのオペランドを生成する
    /// (`[arr index2 else 0]`)
    ///
    /// ## 引数
    /// - index: 添字のノード(`Inst::Mov`か`Inst::Param`)のid
    /// - index_size: 添字の変数の型
    ///
    /// 変数のオペランド(レジスタ/静的領域のラベル)は`extract_operand_text`が
    /// 返すので、レジスタに置かれた変数も、メモリに置かれた変数も同じ
    /// 流れで`ref_arr_indexed`に渡せる。
    pub(in crate::compiler::asm_gen::asm_emitter)
    fn ref_arr_for_var(
        &mut self,
        name: &str,
        index: usize,
        index_size: &Size,
        this_is_self: SelfPtrInfo,
    ) -> String {
        let operand = self.extract_operand_text(index, this_is_self);
        self.ref_arr_indexed(name, &operand, index_size)
    }

    /// 添字が`index_operand`(サイズ`index_size`)にある場合の、
    /// 配列(またはポインタ)`name`の添字付きアドレッシングのオペランドを作る
    ///
    /// 添字を載せた一時レジスタは`self.arr_index_temp`に記録し、
    /// 書き込み後に`write_mem`が解放する
    ///
    /// 流れ: 前回の一時レジスタの解放 → 基準の取得(`arr_ref_base`) →
    /// 添字の読み込み(`load_index_reg`) → 符号の反転と補正(`adjust_index_reg`) →
    /// オペランドの組み立て
    fn ref_arr_indexed(&mut self, name: &str, index_operand: &str, index_size: &Size) -> String {
        // 前回の添字用レジスタが解放されていなければ、ここで解放する
        if let Some(old) = self.arr_index_temp.take() {
            self.used_reg.release(&RegisterMode::Normal(old));
        }

        let ArrRefBase {
            elem_size,
            arr_base,
            is_ptr,
            base,
            arr_len,
        } = self.arr_ref_base(name);

        // 添字を、添字用のレジスタへ読み込む
        let reg_num = self.alloc_reg(&Size::DQ).unwrap_normal();
        self.used_reg.mark_used(&RegisterMode::Normal(reg_num));
        self.arr_index_temp = Some(reg_num);

        let reg64 = self.reg64(&RegisterMode::Normal(reg_num));
        self.load_index_reg(index_operand, index_size, reg_num, &reg64);
        let offset = self.adjust_index_reg(&reg64, is_ptr, arr_len, arr_base, elem_size);
        self.asm_fmt
            .fmt_ref_operand_indexed(&base, &reg64, elem_size, offset)
    }

    /// 配列(またはポインタ)の変数`name`の、添字付きアドレッシングの基準を調べる
    ///
    /// - ローカル配列: ベースは`%rbp`(`dst`のオペランドは`-4(%rbp)`のような
    ///   メモリ参照になるため使わない。使うと`-4(-4(%rbp), ..)`になる)
    /// - ポインタ: ポインタ自身を持つ64bitレジスタがベース
    fn arr_ref_base(&self, name: &str) -> ArrRefBase {
        let var_info = self.var_info(name);
        match &var_info.size {
            Size::Array { size, len } => ArrRefBase {
                elem_size: size.to_bytes(),
                arr_base: var_info.arr_base,
                is_ptr: false,
                base: FRAME_BASE_REG.to_string(),
                arr_len: *len,
            },
            Size::Pointer { ty, .. } => ArrRefBase {
                elem_size: ty.to_bytes(),
                arr_base: 0,
                is_ptr: true,
                base: self.reg64(&var_info.reg),
                arr_len: 0,
            },
            other => ArrRefBase {
                elem_size: other.to_bytes(),
                arr_base: var_info.arr_base,
                is_ptr: false,
                base: FRAME_BASE_REG.to_string(),
                arr_len: 1,
            },
        }
    }

    /// 構造体のメンバー`member`(サイズ`size`)を、レジスタ`reg_num`へ
    /// 64bitに拡張して読み込む。`reg64`は`reg_num`の64bitの名前
    ///
    /// 符号付きの添字は符号拡張(`movs?q`)、符号なしの添字は
    /// ゼロ拡張(`movz?q`)する
    fn load_index_reg(&mut self, member: &str, size: &Size, reg_num: usize, reg64: &str) {
        let unsigned = size.is_unsigned();
        let idx_size = Self::value_reg_size(size);
        match idx_size {
            // 8/16bitの添字は、メモリから直接拡張して読み込む
            Size::DB | Size::DW => {
                let kind = if matches!(idx_size, Size::DB) { 'b' } else { 'w' };
                let load = if unsigned {
                    self.asm_fmt.get_zero_extend(kind, member, reg64)
                } else {
                    self.asm_fmt.get_sign_extend(kind, member, reg64)
                };
                self.asm_text.push_str(&load);
            }
            // 32bitの添字: 32bitで読み込んでから64bitへ拡張する
            //   符号付き:
            //     movl -28(%rbp), %ecx
            //     movslq %ecx, %rcx
            //   符号なし(32bitレジスタへの書き込みで上位32bitは0になるので
            //   movl だけでよい):
            //     movl -28(%rbp), %ecx
            Size::DD => {
                let reg32 = self.asm_fmt.get_fmt_reg(reg_num, &Size::DD);
                let load = self.mov_line(&reg32, member, &Size::DD, true);
                self.asm_text.push_str(&load);
                if !unsigned {
                    let ext = self.asm_fmt.get_sign_extend('l', &reg32, reg64);
                    self.asm_text.push_str(&ext);
                }
            }
            // 64bitの添字: そのまま読み込む
            _ => {
                let load = self.mov_line(reg64, member, &Size::DQ, true);
                self.asm_text.push_str(&load);
            }
        }
    }

    /// 添字のレジスタ`reg64`の符号を反転し、配列なら`要素数 - 1 - 添字`に
    /// 補正して、参照に使うオフセットを返す
    fn adjust_index_reg(
        &mut self,
        reg64: &str,
        is_ptr: bool,
        arr_len: usize,
        arr_base: usize,
        elem_size: usize,
    ) -> usize {
        // 64bitへ符号拡張した添字の符号を反転する
        // (要素はアドレスが小さくなる向きに並んでいる)
        let neg = self.asm_fmt.get_neg(reg64);
        let neg = self.asm_fmt.fmt_mnemonic_resize("neg", &neg, &Size::DQ);
        self.asm_text.push_str(&neg);

        // 配列は添字を `要素数 - 1 - 添字` に反転して、最後の要素
        // (最も%rbpから遠い要素)の位置を基準に参照する
        // (ポインタは要素0を指しているのでオフセットなし)
        if is_ptr {
            return 0;
        }
        if arr_len > 1 {
            let add = self
                .asm_fmt
                .get_add_imm(&self.asm_fmt.get_fmt_num(&(arr_len - 1).to_string()), reg64);
            let add = self.asm_fmt.fmt_mnemonic_resize("add", &add, &Size::DQ);
            self.asm_text.push_str(&add);
        }
        arr_base + elem_size * arr_len.max(1)
    }
}