//! `extract_operand_text`からのみ呼び出すAPI

use super::*;

impl AsmEmitter {
    /// 引数を参照するアセンブリコードの一部
    /// を生成する
    pub(super) 
    fn param_ref(&mut self, param_name: &String) -> String {
        let var_info = self.var_hash_map.get(&param_name.to_string()).unwrap();

        if let Some(ty) = var_info.size.is_pointer() {
            // ポインタ型はアドレス(常に8byte)を保持するため、
            // 64bitレジスタ(`%rcx`など)を経由してメモリを参照する
            let reg = self.asm_fmt.get_fmt_reg(var_info.reg, &Size::DQ);
            self.asm_fmt.fmt_ref_operand(&reg, ty.to_bytes())
        } else {
            self.asm_fmt.get_fmt_reg(var_info.reg, &var_info.size)
        }
    }
    /// 文字列のメモリ参照を生成する
    pub(super) 
    fn string_mem_ref(&mut self, parent_id: usize) -> String {
        let label = self
            .data_map
            .iter()
            .find(|v| v.0 == parent_id)
            .unwrap()
            .1
            .clone();
        self.asm_fmt.fmt_static_var_rip(&label)
    }

    /// 配列に値を代入するコードを生成
    pub(super) 
    fn insert_arr_txt(
        &mut self,
        name: &String,
        dst: usize,
        index: usize,
        this_is_self: &SelfPtrInfo,
    ) -> String {
        let index_value = match self.curr_inst[index].clone() {
            inst::Inst::Num { value, .. } => value
                .parse::<usize>()
                .expect("配列の添字は数字である必要があります"),
            // 配列の添字が構造体のメンバ変数の場合
            inst::Inst::RefStruct { src, pos, size } => {
                // 構造体からindexを取り出す処理を追加
                return self.ref_arr_for_struct_member(name, &src, pos, &size, dst, this_is_self);
            }
            t => panic!("配列の添字には数字のノードが必要です: {:?}", t),
        };
        let var_info = self.var_hash_map.get(&name.to_string()).unwrap();
        if let Some(pointee) = var_info.size.is_pointer() {
            // ポインタは要素0を指し、要素`i`は`ポインタ - i * サイズ`
            let pos = pointee.to_bytes() * index_value;
            let base = self.extract_operand_text(dst, &this_is_self);
            if pos == 0 {
                self.asm_fmt.fmt_ref_operand_no_offset(&base)
            } else {
                self.asm_fmt.fmt_ref_operand(&base, pos)
            }
        } else {
            // 要素`i`は`%rbp - (arr_base + (i + 1) * size)`に置かれる
            // (添字が変数の場合のアドレス計算と同じ配置)
            let size = var_info.size.to_bytes();
            let pos = var_info.arr_base + size * (index_value + 1);
            let base = self.extract_operand_text(dst, &this_is_self);
            self.asm_fmt.fmt_ref_operand(&base, pos)
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
    pub(super) fn ref_struct_txt(&mut self, src: &str, pos: usize, member_size: usize) -> String {
        let size = pos.saturating_sub(member_size);
        let var_info = self.var_hash_map.get(&src.to_string()).expect(&src);

        if var_info.is_stack {
            // `src`(構造体変数自身)が、ポインタをレジスタに持つのでは
            // なく`%rbp`相対のメモリに直接置かれている場合
            // (`a: Name = Name::new()`のように構造体を直接ローカル
            //  変数として初期化した場合など)。
            // このとき`var_info.reg`には`%rbp`からの構造体先頭の
            // オフセットが入っているので、そこにメンバーのオフセット
            // (`size`)を足した位置を直接`%rbp`相対で参照する
            // (レジスタを経由した間接参照`(%rcx)`にはしない)
            let offset = var_info.reg + size;
            self.asm_fmt.fmt_ref_operand(&"%rbp".to_string(), offset)
        } else {
            let reg = self.asm_fmt.get_fmt_reg(var_info.reg, &Size::DQ);
            if size == 0 {
                // 先頭のメンバーは`(%rdi)`のようにオフセットなし
                self.asm_fmt.fmt_ref_operand_no_offset(&reg)
            } else {
                self.asm_fmt.fmt_ref_operand(&reg, size)
            }
        }
    }

    pub(super) 
    fn gen_mov_code(&mut self, name: Option<&str>, src: usize) -> String {
        let var_name = name.clone().unwrap();
        let var = self
            .var_hash_map
            .get(&*var_name)
            .unwrap_or_else(|| panic!("this var is not found -> {}", var_name));

        if var.is_stack {
            return self.asm_fmt.fmt_ref_operand(&"%rbp".to_string(), var.reg);
        }
        let (reg_num, is_ptr, var_size) =
            (var.reg, var.size.is_pointer().is_some(), var.size.clone());
        let size = if is_ptr { Size::DQ } else { var_size };
        if is_ptr == false {
            if let Some(static_var) = self.data_map.iter().find(|v| v.0 == src) {
                // static領域の変数を返す:
                return static_var.1.clone();
            }
        }

        // (以前はここで`self.reg_idx`を変数のレジスタへ書き換えていたため、
        //  `x + 1`のような式の結果が、変数`x`自身のレジスタへ
        //  書き込まれて`x`を壊していた。式の結果のレジスタは
        //  `alloc_reg`で別に確保するので、ここでは何もしない)
        self.asm_fmt.get_fmt_reg(reg_num, &size)
    }

    /// メモリを参照するコードを生成する
    pub(super) fn ref_mem_value_txt(
        &mut self,
        kind: &inst::MemoryKind,
        size: &Size,
        parent_id: usize,
    ) -> String {
        if matches!(kind, inst::MemoryKind::Static) {
            // 静的領域の変数: データセクションに置いたラベルを参照する
            let name = self
                .data_map
                .iter()
                .find(|v| v.0 == parent_id)
                .expect("static var label not found")
                .1
                .clone();
            self.asm_fmt.fmt_static_var_rip(&name)
        } else {
            // スタック領域の変数: %rbpからのオフセットを参照する
            self.asm_fmt
                .fmt_ref_operand(&"%rbp".to_string(), size.to_bytes())
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
        this_is_self: &SelfPtrInfo,
    ) -> String {
        // 代入する先が構造体などの自身のポインタの場合、引数のレジスタにする
        let assign_reg = if this_is_self.is_none() {
            self.asm_fmt.get_fmt_param::<String>(0, &Size::DQ)
        } else {
            "%rbp".to_string()
        };

        let first_id = ids.first().unwrap();

        // 配列の要素のサイズは先頭の要素から求める
        // (配列の要素は全て同じ型/サイズであることが前提)
        let size = match &self.curr_inst[*first_id] {
            inst::Inst::Num { size, .. } => size.clone(),
            t => panic!("配列の要素には数字のノードが必要です: {:?}", t),
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
            let value = self.extract_operand_text(*id, &this_is_self);

            let dst = self
                .asm_fmt
                .fmt_ref_operand(&assign_reg, arr_base + (k + 1) * elem_bytes);

            let mov_line = self
                .asm_fmt
                .get_opcode_tmpl("mov")
                .replace("{dst}", &dst)
                .replace("{src1}", value.as_str());

            txt.push_str(
                self.asm_fmt
                    .fmt_memory_mnemonic_resize("mov", &mov_line, &size)
                    .as_str(),
            );
        }

        if RET_IS_ASM {
            return txt;
        }
        // サイズがSelfポインタでない場合
        if this_is_self.is_some() {
            self.asm_text.push_str(txt.as_str());
        }
        self.asm_fmt
            .fmt_ref_operand(&assign_reg, head_offset)
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
