use crate::compiler::asm_gen::asm_emitter::{VarIndexInfo, RESERVED_REGS};
//use crate::ir::def_tree::{VarMetaData, VarTree};

use super::*;

impl AsmEmitter {
    /// 渡されたテンプレート文字列(オペランドの`{0}`などを置換する前の、
    /// 生のインラインアセンブラの1行)の中に、直接書かれているレジスタ名
    /// (`%rax`など)を検出し、対応する内部のレジスタ番号を返す。
    fn detect_literal_regs(&self, template: &str) -> Vec<usize> {
        let mut found = Vec::new();
        self.asm_fmt.all_reg_names().iter().for_each(|(reg_idx, reg_name)| {
            if template.contains(reg_name.as_str()) && !found.contains(reg_idx) {
                found.push(*reg_idx);
            }
        });
        found
    }

    /// `exclude`(インラインアセンブラ自身が使うレジスタ)を除いた上で、
    /// 現在使われていない、空いているレジスタ番号を1つ探す。
    ///
    /// `%rax`/`%rdx`(`RESERVED_REGS`)と、まだ定義されていない変数のために
    /// 取り置かれているレジスタは選ばない。
    fn find_free_reg(&self, exclude: &[usize]) -> Option<usize> {
        (0..self.asm_fmt.reg_count()).find(|reg| {
            !RESERVED_REGS.contains(reg)
                && !exclude.contains(reg)
                && !self.used_reg.is_used(*reg)
                && !self.used_reg.is_reserved(*reg)
        })
    }

    /// インラインアセンブラが暗黙に書き換えるレジスタ
    /// (`syscall`は`%rcx`と`%r11`を書き換える)
    fn implicit_clobbered_regs(&self, lines: &[(String, Vec<usize>)]) -> Vec<usize> {
        if !lines.iter().any(|(template, _)| template.contains("syscall")) {
            return Vec::new();
        }
        self.asm_fmt
            .all_reg_names()
            .iter()
            .filter(|(_, name)| name == "%rcx" || name == "%r11")
            .map(|(reg, _)| *reg)
            .collect()
    }

    fn resolve_operand_var_size(&self, node_idx: usize) -> Option<Size> {
        match self.inst_at(node_idx) {
            inst::Inst::AssignVar { name, .. } => self.var_size(&name),
            inst::Inst::Param(param) => self.var_size(&param.name),
            inst::Inst::Pointer(inner) | inst::Inst::GetAddress(inner) => {
                self.resolve_operand_var_size(inner)
            }
            _ => None,
        }
    }

    /// インラインアセンブラ内の`${var}`が、ポインタ型の引数(`Inst::Param`)
    /// を指している場合のオペランドを返す。
    fn inline_ptr_param_operand(&self, node_idx: usize) -> Option<String> {
        let inst::Inst::Param(param) = &self.curr_inst[node_idx] else {
            return None;
        };
        let var_info = self.find_var_info(&param.name)?;
        var_info.size.is_pointer()?;

        Some(self.ptr_deref_operand(var_info.reg))
    }

    fn extract_operand_text_sized(
        &mut self,
        node_idx: usize,
        this_is_self: SelfPtrInfo,
        forced_size: &Size,
    ) -> String {
        match self.inst_at(node_idx) {
            inst::Inst::AssignVar { name, .. } => {
                let var_info = self.var_info(&name);
                if var_info.is_stack {
                    // レジスタを持たない変数は、`%rbp`相対のメモリ参照にする
                    return self.var_operand(&name);
                }
                let size = gen_reg_size(&var_info, &forced_size);
                self.asm_fmt.get_fmt_reg(var_info.reg, &size)
            }
            inst::Inst::Param(param) => {
                let var_info = self.var_info(&param.name);
                if var_info.size.is_pointer().is_some() {
                    self.ptr_deref_operand(var_info.reg)
                } else {
                    self.asm_fmt.get_fmt_reg(var_info.reg, forced_size)
                }
            }
            // それ以外(メモリ参照/即値/ポインタなど)は、サイズの
            // 上書きを行わず通常通り組み立てる
            _ => self.extract_operand_text(node_idx, this_is_self),
        }
    }

    pub(super) 
    fn deploy_inline_asm(
        &mut self, 
        name: &str, 
        lines: &[(String, Vec<usize>)]
    ) {
        if self
            .asm_fmt
            .inline_asm_list()
            .iter()
            .find(|v| v.as_str() == name)
            .is_some()
        {
            // === 1. インラインアセンブラ中で使われているレジスタを検出する ===
            let mut literal_regs = Vec::<usize>::new();
            lines.iter().for_each(|(template, _)| {
                self.detect_literal_regs(template).iter().for_each(|reg| {
                    if !literal_regs.contains(&reg) {
                        literal_regs.push(*reg);
                    }
                });
            });
            // `syscall`などが暗黙に書き換えるレジスタも、使われているものとして扱う
            self.implicit_clobbered_regs(lines).into_iter().for_each(|reg| {
                if !literal_regs.contains(&reg) {
                    literal_regs.push(reg);
                }
            });

            // === 2. 使用中のレジスタは、空いている別のレジスタへ移す ===
            // (`push`では、`${msg}`などの埋め込み先が書き換えられたレジスタの
            //  ままになってしまう。移した先のレジスタを`${..}`に埋め込む)
            // 移した (元のレジスタ, 移した先のレジスタ)
            let mut moved_regs: Vec<(usize, usize)> = Vec::new();
            // 空きレジスタが無く、スタックへ退避した(後で必ずpopする)レジスタ
            let mut stacked_regs: Vec<usize> = Vec::new();
            literal_regs.iter().for_each(|reg| {
                if self.used_reg.is_used(*reg) {
                    match self.mov_register_val_to_reg(*reg, &literal_regs) {
                        Some(free_reg) => moved_regs.push((*reg, free_reg)),
                        None => self.mov_register_val_to_stack(*reg, &mut stacked_regs),
                    }
                }
            });

            // === インラインアセンブラ本体を展開する ===
            self.gen_inline_asm_txt(&lines);

            // === 3. 移したレジスタの値を、元のレジスタへ戻す ===
            // (インラインアセンブラが元のレジスタを別の値で書き換えているので、
            //  この後のコードのために変数の位置を元に戻す)
            moved_regs.iter().rev().for_each(|(reg, free_reg)| {
                self.restore_register_val(*reg, *free_reg);
            });
            // === スタックに退避したレジスタを、退避した時とは逆順に戻す ===
            stacked_regs.iter().rev().for_each(|reg| {
                let reg_name = self.reg64(*reg);
                let pop_asm = self.asm_fmt.get_pop(&reg_name);
                self.asm_text.push_str(&pop_asm);
            });
        } else {
            panic!();
        }
    }

    /// レジスタ`reg`の値(変数・式の一時的な結果)を、インラインアセンブラが
    /// 使わない空いているレジスタへ`mov`し、`reg`を指していた変数と式の結果の
    /// 参照先を、移した先のレジスタに切り替える。
    ///
    /// 空いているレジスタが無ければ何もせず`None`を返す。
    #[inline(always)]
    fn mov_register_val_to_reg(&mut self, reg: usize, asm_regs: &[usize]) -> Option<usize> {
        let free_reg = self.find_free_reg(asm_regs)?;

        // 上位ビットも含めて、レジスタ全体を移す
        let src = self.reg64(reg);
        let dst = self.reg64(free_reg);
        let mov_asm = self.mov_line(&dst, &src, &Size::DQ, false);
        self.asm_text.push_str(&mov_asm);

        self.retarget_reg(reg, free_reg);
        Some(free_reg)
    }

    /// `mov_register_val_to_reg`で`free_reg`へ移した値を、元の`reg`へ戻す
    #[inline(always)]
    fn restore_register_val(&mut self, reg: usize, free_reg: usize) {
        let src = self.reg64(free_reg);
        let dst = self.reg64(reg);
        let mov_asm = self.mov_line(&dst, &src, &Size::DQ, false);
        self.asm_text.push_str(&mov_asm);

        self.retarget_reg(free_reg, reg);
        // 一時的に使った`free_reg`を解放する
        self.used_reg.release(free_reg);
    }

    /// レジスタ`from`を保持している変数と式の結果を、`to`を保持するように切り替える
    fn retarget_reg(&mut self, from: usize, to: usize) {
        let names: Vec<String> = self
            .var_hash_map
            .iter()
            .filter(|(_, var)| !var.is_stack && var.reg == from)
            .map(|(name, _)| name.clone())
            .collect();
        names.iter().for_each(|name| self.update_value_reg(name, to));
        self.last_inst_idx
            .iter_mut()
            .filter(|(_, reg)| *reg == from)
            .for_each(|(_, reg)| *reg = to);
        self.used_reg.mark_used(to);
    }

    /// レジスタが空いていない場合に、スタックに退避する
    #[inline(always)]
    fn mov_register_val_to_stack(&mut self, reg: usize, stacked_regs: &mut Vec<usize>) {
        let reg_name = self.reg64(reg);
        let push_asm = self.asm_fmt.get_push(&reg_name);
        self.asm_text.push_str(&push_asm);
        stacked_regs.push(reg);
    }

    #[inline(always)]
    fn gen_inline_asm_txt(&mut self, lines: &[(String, Vec<usize>)]) {
        // === インラインアセンブラ本体を展開する ===
        for (template, operand_ids) in lines.iter() {
            let mut asm_line = template.clone();

            // AT&T記法の慣例により、最後のオペランドをdstとして扱う。
            // srcのレジスタサイズは、常にこのdstのサイズに合わせる
            // (dstが変数を参照していない場合は上書きしない)
            let dst_size = operand_ids
                .last()
                .and_then(|id| self.resolve_operand_var_size(*id));
            let last_index = operand_ids.len().saturating_sub(1);

            for (index, operand_id) in operand_ids.iter().enumerate() {
                let operand_text = if let Some(ptr_operand) =
                    // ポインタ型の引数は、オフセット無しの参照にする
                    self.inline_ptr_param_operand(*operand_id)
                {
                    ptr_operand
                } else {
                    match (index != last_index, &dst_size) {
                        (true, Some(size)) => {
                            self.extract_operand_text_sized(*operand_id, dst_size.as_ref(), size)
                        }
                        _ => self.extract_operand_text(*operand_id, dst_size.as_ref()),
                    }
                };
                // `{0}`, `{1}`, ... という数字のプレースホルダーを置換
                // (`${var}`はパーサー側(preproc.rs)の時点で既に
                //  `{index}`へ変換済みのため、ここでは数字の
                //  プレースホルダーだけを見れば良い)
                asm_line = asm_line.replace(&format!("{{{}}}", index), &operand_text);
            }

            asm_line.push('\n');
            self.asm_text.push_str(&asm_line);
        }
    }
}

#[inline(always)]
fn gen_reg_size(var_info: &VarIndexInfo, forced_size: &Size) -> Size {
    if var_info.size.is_pointer().is_some() {
        Size::DQ
    } else {
        forced_size.clone()
    }
}