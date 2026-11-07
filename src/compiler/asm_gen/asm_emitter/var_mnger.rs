use super::*;

#[derive(Debug, Clone)]
pub struct VarIndexInfo {
    /// レジスタ番号。`is_stack`が`true`の変数(ポインタでない構造体の
    /// 変数)はレジスタを持たないので[`VarIndexInfo::NO_REG`]が入る
    /// (`%rbp`からの位置は`offset`を使う)
    pub reg: RegisterMode,
    /// `is_stack`が`true`の場合の、`%rbp`からのバイトオフセット
    /// (構造体先頭のメンバーの位置)。`is_stack`が`false`の場合は`0`で意味を持たない
    pub offset: usize,
    pub size: types::Size,
    pub index: usize,
    pub is_stack: bool,
    /// 配列変数の場合の、要素0の位置(`%rbp`からのバイトオフセット。領域の一番下)。
    /// 要素`i`は`%rbp - arr_base + i * 要素サイズ`に置かれる
    /// (上向き。ポインタの添字`ptr + i * 要素サイズ`と同じ向き)。配列以外では`0`
    pub arr_base: usize,
    /// スタック上のメモリ(配列など、`mem_val_ir`で確保した領域)に
    /// 実体がある変数かどうか。`true`の場合、`reg`はレジスタ番号として
    /// 使われていないので、レジスタの使用中の管理や、インラインアセンブラの
    /// レジスタ退避の対象にしてはいけない
    pub in_mem: bool,
}

impl VarIndexInfo {
    /// レジスタを持たない変数(`is_stack`)の`reg`に入れる値。
    /// レジスタ番号として使うと範囲外になって即座にpanicするので、
    /// オフセットをレジスタ番号と取り違えるバグを見逃さない
    pub const NO_REG: usize = usize::MAX;

    pub fn new(reg: RegisterMode, size: &types::Size, index: usize) -> Self {
        Self {
            reg: reg,
            offset: 0,
            size: size.clone(),
            index: index,
            is_stack: false,
            arr_base: 0,
            in_mem: false,
        }
    }

    /// ポインタでない構造体の変数を、`%rbp`からのオフセット(`offset`)に
    /// 直接置かれている変数として登録する。レジスタは使わない
    pub fn new_stack(offset: usize, size: &types::Size, index: usize) -> Self {
        Self {
            reg: RegisterMode::Normal(Self::NO_REG),
            offset,
            size: size.clone(),
            index: index,
            is_stack: true,
            arr_base: 0,
            in_mem: false,
        }
    }

    /// 静的領域(`MemoryKind::Static`)に実体がある変数として登録する。
    /// 値はラベル(`M0(%rip)`)で参照するので、レジスタは持たない
    ///
    /// `reg`は、オペランドを引く処理が範囲外にならないための値にすぎず、レジスタを
    /// 保持しているわけではない(`in_mem`と同じ扱い。使用中にも記録しない)。
    /// 以前は`reg`に`reg_idx`を入れて通常の変数として登録していたため、そのレジスタが
    /// 使用中のまま解放されず、同じ番号の一時的な結果(静的変数のアドレスなど)も
    /// 「変数が持っている」と見なされて解放されなかった
    pub fn new_static(reg: RegisterMode, size: &types::Size, index: usize) -> Self {
        Self {
            reg,
            offset: 0,
            size: size.clone(),
            index: index,
            is_stack: false,
            arr_base: 0,
            in_mem: true,
        }
    }

    /// この変数が、`reg`のレジスタを実際に保持しているか
    /// (`%rbp`相対のメモリ・メモリ上の実体(配列/静的領域)の変数は保持しない)
    pub fn holds_reg(&self) -> bool {
        !self.is_stack && !self.in_mem
    }

    /// 配列の先頭の位置(`%rbp`からのオフセット)を記録する
    pub fn with_arr_base(mut self, arr_base: usize) -> Self {
        self.arr_base = arr_base;
        self.in_mem = true;
        self
    }
}

/// レジスタが足りずスタックへ置く変数の、初期値の受け渡しに使うレジスタ
/// (`%rax`。`RESERVED_REGS`なので、変数や式の結果には割り当てられない)
const SPILL_SCRATCH: usize = 0;
/// スタックへ置いたポインタ変数が指す先を読み書きするとき、ポインタの値を
/// 一時的に読み込むレジスタ(`%rdx`。`RESERVED_REGS`)
const SPILL_PTR_SCRATCH: usize = 2;

impl AsmEmitter {
    /// 指定された名前の変数の型を返す
    pub(in crate::compiler::asm_gen) fn get_var_ty(
        &self,
        var_name: &str,
    ) -> Result<Size, err::ErrKind> {
        Ok(self.get_var_all_info(var_name)?.size.clone())
    }

    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn update_value_info(&mut self, name: &str, index: usize) {
        self.var_info_mut(name).index = index;
    }

    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn update_value_reg(
        &mut self,
        name: &str,
        reg: &RegisterMode,
    ) {
        self.used_reg.mark_used(reg);
        self.var_info_mut(name).reg = reg.clone();
    }

    /// レジスタに載せる値の型`ty`から、実際に使うレジスタの
    /// サイズを決める。
    ///
    /// - `DB`/`DW`/`DD`/`DQ`: そのままのサイズ
    /// - 配列: 要素のサイズ
    /// - ポインタ/構造体など: アドレスを持つので64bit(`DQ`)
    pub(in crate::compiler::asm_gen) fn value_reg_size(ty: &Size) -> Size {
        match ty {
            Size::DB | Size::DW | Size::DD | Size::DQ | Size::F32 | Size::F64 => ty.clone(),
            // 符号なしは、同じ幅の符号付きと同じレジスタを使う
            Size::UDB | Size::UDW | Size::UDD | Size::UDQ => ty.to_signed(),
            Size::Array { size, .. } => Self::value_reg_size(size),
            _ => Size::DQ,
        }
    }

    /// 変数1つ分(8byte)のスタック領域を確保し、`%rbp`からのオフセットを返す
    /// (`stk_use_counter`を進める。先にある配列などと重ならない)
    fn alloc_var_slot(&mut self) -> usize {
        let offset = self.fn_state.stk_use_counter.div_ceil(8) * 8 + 8;
        self.fn_state.stk_use_counter = offset;
        offset
    }

    /// 式の結果または変数を保持しているレジスタをスタックへ退避する
    pub(in crate::compiler::asm_gen) fn spill_reg_to_stack(
        &mut self,
        reg: RegisterMode,
    ) -> Result<bool, err::ErrKind> {
        let var_names: Vec<String> = self
            .fn_state
            .var_hash_map
            .iter()
            .filter(|(name, var)| {
                !self.fn_state.dead_vars.contains(name.as_str())
                    && !var.is_stack
                    && !var.in_mem
                    && var.reg == reg
            })
            .map(|(name, _)| name.clone())
            .collect();
        let expr_ids: Vec<usize> = self
            .fn_state
            .last_inst_idx
            .iter()
            .filter(|(_, result_reg)| *result_reg == reg)
            .map(|(id, _)| *id)
            .collect();
        if var_names.is_empty() && expr_ids.is_empty() {
            if self.fn_state.spilled_regs.remove(&reg).is_some() {
                self.used_reg.release(&reg);
                return Ok(true);
            }
            return Ok(false);
        }

        if self.fn_state.spilled_regs.contains_key(&reg) && !expr_ids.is_empty() {
            return Ok(false);
        }
        let spilled_offset = self.fn_state.spilled_regs.remove(&reg);
        let offset = match spilled_offset {
            Some(offset) => offset,
            None => {
                let offset = self.alloc_var_slot();
                let slot = self.rbp_ref(offset);
                let store = match reg {
                    RegisterMode::Normal(_) => {
                        self.mov_line(&slot, &self.reg64(&reg)?, &Size::DQ, true)
                    }
                    RegisterMode::Float(_) => {
                        let size = self.float_reg_size(reg);
                        let opcode = Self::float_move_opcode(&size);
                        self.asm_fmt
                            .get_opcode_tmpl(opcode)
                            .replace("{src1}", &self.reg_mode_operand(reg, &size)?)
                            .replace("{dst}", &slot)
                    }
                };
                self.asm_text.push_str(&store);
                offset
            }
        };

        for name in var_names {
            let var = self.var_info_mut(&name);
            var.reg = RegisterMode::Normal(VarIndexInfo::NO_REG);
            var.offset = offset;
            var.is_stack = true;
        }
        for id in expr_ids {
            self.fn_state
                .last_inst_idx
                .retain(|(node_id, _)| *node_id != id);
            self.fn_state.spilled_expr_results.push((id, offset));
        }
        self.used_reg.release(&reg);
        Ok(true)
    }

    /// レジスタが足りない変数が、スタックに置かれているスカラー(整数・浮動小数点・
    /// ポインタ)か(構造体の変数は含まない)
    pub(in crate::compiler::asm_gen) fn is_spilled_scalar(&self, name: &str) -> bool {
        self.find_var_info(name)
            .is_some_and(|var| var.is_stack && !matches!(var.size, Size::Struct(..)))
    }

    /// 値を`dst_operand`(`-8(%rbp)`などのメモリ参照)へ書き込む。
    ///
    /// メモリからメモリへは`mov`できないので、値を一時レジスタ(整数は`%rax`)へ
    /// 書く命令を`init`に作らせ、そのレジスタからメモリへ書き込む。
    /// 浮動小数点は、一時レジスタに使う`xmm`の最後のレジスタを、書き込みの前後で
    /// スタックへ退避・復元する(全て変数が使っている可能性があるため)。
    ///
    /// ## 引数
    /// - init: 値を一時レジスタ(引数)へ書き込む命令を返す
    fn store_via_scratch(
        &mut self,
        size: &Size,
        dst_operand: &str,
        init: impl FnOnce(&mut Self, RegisterMode) -> Result<String, err::ErrKind>,
    ) -> Result<(), err::ErrKind> {
        let store_size = Self::value_reg_size(size);
        if size.is_float() {
            let scratch = RegisterMode::Float(self.asm_fmt.float_reg_count() - 1);
            let opcode = Self::float_move_opcode(&store_size);
            // レジスタの中身(`F32`でも`F64`でも)を壊さないよう、64bitで退避する
            let save_opcode = Self::float_move_opcode(&Size::F64);
            let save_offset = self.alloc_var_slot();
            let save_ref = self.rbp_ref(save_offset);
            let save_reg = self.reg_mode_operand(scratch, &Size::F64)?;
            let save = self.fill_tmpl(save_opcode, &save_ref, &save_reg);
            let restore = self.fill_tmpl(save_opcode, &save_reg, &save_ref);
            self.asm_text.push_str(&save);

            let load = init(self, scratch)?;
            self.asm_text.push_str(&load);
            let scratch_txt = self.reg_mode_operand(scratch, &store_size)?;
            let store = self.fill_tmpl(opcode, dst_operand, &scratch_txt);
            self.asm_text.push_str(&store);
            self.asm_text.push_str(&restore);
        } else {
            let scratch = RegisterMode::Normal(SPILL_SCRATCH);
            let load = init(self, scratch)?;
            self.asm_text.push_str(&load);
            let scratch_txt = self.reg_mode_operand(scratch, &store_size)?;
            let store = self.mov_line(dst_operand, &scratch_txt, &store_size, true);
            self.asm_text.push_str(&store);
        }
        Ok(())
    }

    /// レジスタが足りなくなった変数を、スタックに置いて変数として登録する
    ///
    /// 変数用のスタック領域を確保し、初期値を書き込んで、
    /// `%rbp`からのオフセットを持つ変数として登録する(`VarIndexInfo::new_stack`)。
    /// 以降の参照は`var_operand`が`-8(%rbp)`のメモリ参照にする。
    ///
    /// ## 引数
    /// - dst: 変数のIRのノードid(登録する`index`)
    /// - init: 初期値を一時レジスタ(引数)へ書き込む命令を返す
    pub(in crate::compiler::asm_gen) fn define_spilled_var(
        &mut self,
        size: &Size,
        dst: usize,
        name: Option<&str>,
        init: impl FnOnce(&mut Self, RegisterMode) -> Result<String, err::ErrKind>,
    ) -> Result<(), err::ErrKind> {
        let offset = self.alloc_var_slot();
        let slot = self.rbp_ref(offset);
        self.store_via_scratch(size, &slot, init)?;
        if let Some(var_name) = name {
            self.insert_var_info(var_name, VarIndexInfo::new_stack(offset, size, dst));
        }
        Ok(())
    }

    /// スタックに置いた変数`name`へ、再代入する
    pub(in crate::compiler::asm_gen) fn assign_spilled_var(
        &mut self,
        name: &str,
        init: impl FnOnce(&mut Self, RegisterMode) -> Result<String, err::ErrKind>,
    ) -> Result<(), err::ErrKind> {
        let var = self.get_var_all_info(name)?;
        let (size, slot) = (var.size.clone(), self.rbp_ref(var.offset));
        self.store_via_scratch(&size, &slot, init)
    }

    /// 構造体・バリアントの初期化が、暗黙の`self`ポインタを使いながら関数も呼ぶか
    /// (呼び出しで第1引数のレジスタが上書きされるので、先に退避する必要がある)
    pub(in crate::compiler::asm_gen) fn needs_self_ptr_save(&self) -> bool {
        let has_call = self
            .fn_state
            .curr_inst
            .iter()
            .any(|node| matches!(node, inst::Inst::CallFunc(..)));
        let has_init = self.fn_state.curr_inst.iter().any(|node| {
            matches!(
                node,
                inst::Inst::Struct { .. } | inst::Inst::Variant { .. } | inst::Inst::InitArr(..)
            )
        });
        has_call && has_init
    }

    /// `self`ポインタ(第1引数のレジスタ)を、スタックの専用の領域へ退避する命令を作る
    /// (退避先を`self_ptr_slot`に記録する)
    pub(in crate::compiler::asm_gen) fn save_self_ptr_asm(&mut self) -> String {
        let offset = self.alloc_var_slot();
        self.fn_state.self_ptr_slot = Some(offset);
        self.mov_line(&self.rbp_ref(offset), &self.self_ptr_reg(), &Size::DQ, true)
    }

    /// `self`ポインタをベースにして書き込むための、(読み込む命令, ベースのレジスタ名)
    ///
    /// 退避していれば、退避先から一時レジスタ(`%rdx`)へ読み込む命令と`%rdx`を返す
    /// (呼び出しで上書きされた第1引数のレジスタは使わない)。
    /// 退避していなければ、命令は空で、第1引数のレジスタを返す。
    /// 読み込む命令は、そのレジスタを使う命令の直前に置くこと
    pub(in crate::compiler::asm_gen) fn self_ptr_base(
        &self,
    ) -> Result<(String, String), err::ErrKind> {
        match self.fn_state.self_ptr_slot {
            Some(offset) => {
                let scratch = self.reg64(&RegisterMode::Normal(SPILL_PTR_SCRATCH))?;
                let load = self.mov_line(&scratch, &self.rbp_ref(offset), &Size::DQ, true);
                Ok((load, scratch))
            }
            None => Ok((String::new(), self.self_ptr_reg())),
        }
    }

    /// ポインタ変数`name`の値を持つ64bitレジスタ名を返す
    /// (指す先を`(%reg)`で参照するときのベースに使う)
    ///
    /// スタックに置いたポインタ変数は、一時レジスタ(`%rdx`)へ読み込む命令を
    /// 先に出力してから、そのレジスタ名を返す
    pub(in crate::compiler::asm_gen) fn ptr_var_base(
        &mut self,
        name: &str,
    ) -> Result<String, err::ErrKind> {
        let var = self.get_var_all_info(name)?;
        let (is_stack, offset, reg) = (var.is_stack, var.offset, var.reg);
        if !is_stack {
            return self.reg64(&reg);
        }
        let scratch = self.reg64(&RegisterMode::Normal(SPILL_PTR_SCRATCH))?;
        let load = self.mov_line(&scratch, &self.rbp_ref(offset), &Size::DQ, true);
        self.asm_text.push_str(&load);
        Ok(scratch)
    }
}
