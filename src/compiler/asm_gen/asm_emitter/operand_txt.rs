//! `extract_operand_text`からのみ呼び出すAPI

use super::*;

impl AsmEmitter {
    /// 引数を参照するアセンブリコードの一部
    /// を生成する
    pub(super) fn param_ref(&mut self, param_name: &str) -> Result<String, err::ErrKind> {
        let var_info = self.get_var_all_info(param_name)?;

        if let Some(ty) = var_info.size.is_pointer() {
            // ポインタ型はアドレス(常に8byte)を保持するため、
            // 64bitレジスタ(`%rcx`など)を経由してメモリを参照する
            let reg = self.reg64(&var_info.reg)?;
            Ok(self.asm_fmt.fmt_ref_operand(&reg, ty.to_bytes()))
        } else {
            // Result型を返している
            match var_info.reg {
                RegisterMode::Float(idx) => self.asm_fmt.get_fmt_float_reg(idx, &var_info.size),
                RegisterMode::Normal(idx) => self.asm_fmt.get_fmt_reg(idx, &var_info.size),
            }
        }
    }
    /// 文字列のメモリ参照を生成する
    pub(super) fn string_mem_ref(&mut self, parent_id: usize) -> Result<String, err::ErrKind> {
        let label = self
            .static_label(parent_id)
            .ok_or_else(|| err::ErrKind::asm_emit_label_not_found("string", parent_id))?;
        Ok(self.asm_fmt.fmt_static_var_rip(&label))
    }

    /// 配列に値を代入するコードを生成
    pub(super) fn insert_arr_txt(
        &mut self,
        name: &str,
        dst: usize,
        index: usize,
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        let index_value = match self.inst_at(index) {
            inst::Inst::Num { value, .. } => value
                .parse::<usize>()
                .map_err(|_| err::ErrKind::asm_emit_array_index_not_number(value.to_string()))?,
            // 配列の添字が構造体のメンバ変数の場合
            inst::Inst::RefStruct { src, pos, size } => {
                // 構造体からindexを取り出す処理を追加
                return self.ref_arr_for_struct_member(name, &src, pos, &size, dst, this_is_self);
            }
            // 配列の添字が普通の変数の場合(`[arr index2 else 0]`)
            // ローカル変数
            inst::Inst::Mov {
                name: Some(_),
                size: index_size,
                ..
            } => {
                return self.ref_arr_for_var(name, index, &index_size, this_is_self);
            }
            // 関数の引数
            inst::Inst::Param(param) => {
                return self.ref_arr_for_var(name, index, &param.ty, this_is_self);
            }
            t => {
                return Err(err::ErrKind::asm_emit_invalid_array_index_node(format!(
                    "{:?}",
                    t
                )));
            }
        };
        let var_info = self.get_var_all_info(name)?;
        if let Some(pointee) = var_info.size.is_pointer() {
            // ポインタは要素0を指し、要素`i`は`ポインタ - i * サイズ`
            let pos = pointee.to_bytes() * index_value;
            let base = self.extract_operand_text(dst, this_is_self)?;
            Ok(self.ref_base_offset(&base, pos))
        } else {
            // 要素`i`は`%rbp - (arr_base + (i + 1) * size)`に置かれる
            // (添字が変数の場合のアドレス計算と同じ配置)
            //
            // ベースは常に`%rbp`にする。`dst`のオペランドは配列自身を指す
            // `-4(%rbp)`のようなメモリ参照なので、ベースに使うと
            // `-4(-4(%rbp))`になってしまう(`arr_ref_base`と同じ理由)
            let size = var_info.size.to_bytes();
            let pos = var_info.arr_base + size * (index_value + 1);
            Ok(self.rbp_ref(pos))
        }
    }

    /// 構造体のメンバーを参照するコードを作成
    ///
    /// 構造体のポインタは先頭のメンバーを指し、メンバーは
    /// ポインタから見てアドレスが小さくなる向きに並ぶ
    /// (`a: int`, `b: int`なら`(%rdi)`と`-4(%rdi)`)。
    ///
    /// ## 引数
    /// - pos = IRの、そのメンバーの終端までの累積サイズ
    /// - member_size = そのメンバー自身のサイズ
    ///   (`pos - member_size`が、ポインタからのオフセットになる)
    pub(super) fn ref_struct_txt(
        &mut self,
        src: &str,
        pos: usize,
        member_size: usize,
    ) -> Result<String, err::ErrKind> {
        let size = Self::struct_member_offset(pos, member_size);
        let var_info = self.get_var_all_info(src)?;

        // レジスタが足りずスタックに置いたポインタ変数(`is_stack`だが構造体ではない)は、
        // 構造体変数ではないので、ポインタの値を読み込んだレジスタ経由で参照する
        if var_info.is_stack && !matches!(var_info.size, Size::Struct(..)) {
            let reg = self.ptr_var_base(src)?;
            return Ok(self.ref_base_offset(&reg, size));
        }
        if var_info.is_stack {
            // `src`(構造体変数自身)が、ポインタをレジスタに持つのでは
            // なく`%rbp`相対のメモリに直接置かれている場合
            // (`a: Name = Name::new()`のように構造体を直接ローカル
            //  変数として初期化した場合など)。
            // このとき`var_info.offset`には`%rbp`からの構造体先頭の
            // オフセットが入っているので、そこにメンバーのオフセット
            // (`size`)を足した位置を直接`%rbp`相対で参照する
            // (レジスタを経由した間接参照`(%rcx)`にはしない)
            Ok(self.rbp_ref(var_info.offset + size))
        } else {
            // 先頭のメンバーは`(%rdi)`のようにオフセットなし
            let reg = self.reg64(&var_info.reg)?;
            Ok(self.ref_base_offset(&reg, size))
        }
    }

    pub(super) fn gen_mov_code(
        &mut self,
        name: Option<&str>,
        src: usize,
    ) -> Result<String, err::ErrKind> {
        let var_name = name.ok_or_else(|| err::ErrKind::asm_emit_var_name_missing(src))?;
        let var = self.get_var_all_info(var_name)?;

        // ポインタでない、レジスタ上の変数が static領域の値なら、そのラベルを返す
        if !var.is_stack && var.size.is_pointer().is_none() {
            if let Some(label) = self.static_label(src) {
                return Ok(self.asm_fmt.fmt_static_var_rip(&label));
            }
        }

        // (以前はここで`self.reg_idx`を変数のレジスタへ書き換えていたため、
        //  `x + 1`のような式の結果が、変数`x`自身のレジスタへ
        //  書き込まれて`x`を壊していた。式の結果のレジスタは
        //  `alloc_reg`で別に確保するので、ここでは何もしない)
        self.var_operand(var_name)
    }

    /// メモリを参照するコードを生成する
    pub(super) fn ref_mem_value_txt(
        &mut self,
        kind: &inst::MemoryKind,
        size: &Size,
        parent_id: usize,
    ) -> Result<String, err::ErrKind> {
        if matches!(kind, inst::MemoryKind::Static) {
            // 静的領域の変数: データセクションに置いたラベルを参照する
            let name = self
                .static_label(parent_id)
                .ok_or_else(|| err::ErrKind::asm_emit_label_not_found("static var", parent_id))?;
            Ok(self.asm_fmt.fmt_static_var_rip(&name))
        } else {
            // スタック領域の変数: %rbpからのオフセットを参照する
            Ok(self.rbp_ref(size.to_bytes()))
        }
    }

    /// `Inst::InitArr`(変数に束縛されていない配列リテラル、
    /// 例: 関数の引数にそのまま渡された`{1,2,3}`)を
    /// スタック上に展開し、その先頭要素を指すオペランドを返す
    ///
    /// ## 引数
    /// - ids: 配列の各要素の値を持つノード(`Inst::Num`など)のid
    pub(super) fn init_arr_txt<const RET_IS_ASM: bool>(
        &mut self,
        ids: &[usize],
        this_is_self: SelfPtrInfo,
    ) -> Result<String, err::ErrKind> {
        // 代入する先が構造体などの自身のポインタの場合、引数のレジスタにする
        let assign_reg = if this_is_self.is_none() {
            self.self_ptr_reg()
        } else {
            FRAME_BASE_REG.to_string()
        };

        let first_id = ids
            .first()
            .ok_or_else(err::ErrKind::asm_emit_empty_array_init)?;

        // 配列の要素のサイズは先頭の要素から求める
        // (配列の要素は全て同じ型/サイズであることが前提)
        let size = match self.curr_inst.get(*first_id) {
            Some(inst::Inst::Num { size, .. }) => size.clone(),
            other => {
                return Err(err::ErrKind::asm_emit_invalid_array_elem_node(format!(
                    "{:?}",
                    other
                )));
            }
        };

        let mut txt = String::new();

        // 要素0が最も%rbpに近い位置に来て、要素が増えるほどアドレスが
        // 小さくなる。領域全体を先に確保し、要素`k`のオフセットは
        // `arr_base + (k + 1) * 要素のサイズ`(要素0のオフセットが`head_offset`)
        let elem_bytes = size.to_bytes();
        let arr_base = self.stk_use_counter;
        let head_offset = arr_base + elem_bytes;
        self.stk_use_counter = arr_base + elem_bytes * ids.len();

        for (k, id) in ids.iter().enumerate() {
            let value = self.extract_operand_text(*id, this_is_self)?;

            let dst = self
                .asm_fmt
                .fmt_ref_operand(&assign_reg, arr_base + (k + 1) * elem_bytes);

            txt.push_str(&self.mov_line(&dst, &value, &size, true));
        }

        if RET_IS_ASM {
            return Ok(txt);
        }
        // サイズがSelfポインタでない場合
        if this_is_self.is_some() {
            self.asm_text.push_str(txt.as_str());
        }
        Ok(self.asm_fmt.fmt_ref_operand(&assign_reg, head_offset))
    }

    /// 二行のアセンブリコードのニーモニックを、式のサイズに応じて調整する
    #[inline(always)]
    pub fn fmt_one_expr_mnemo_resize(
        &self,
        mut formated: String,
        resolved_size: &Size,
        mnemonic: &str,
        is_memory_access: bool,
    ) -> String {
        if is_memory_access {
            formated = self
                .asm_fmt
                .fmt_memory_mnemonic_resize("mov", &formated, resolved_size);
            self.asm_fmt
                .fmt_memory_mnemonic_resize(mnemonic, &formated, resolved_size)
        } else {
            formated = self
                .asm_fmt
                .fmt_mnemonic_resize("mov", &formated, resolved_size);
            self.asm_fmt
                .fmt_mnemonic_resize(mnemonic, &formated, resolved_size)
        }
    }
}
