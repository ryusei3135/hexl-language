use crate::compiler::{
    asm_gen::asm_emitter::VarIndexInfo,
    err::{self, undef::UndefKind::UndefInlineAsm},
};
//use crate::ir::def_tree::{VarMetaData, VarTree};

use super::asm_fmt::mng_fmt::RAW_ASM_MARK;
use super::*;

impl AsmEmitter {
    fn all_inline_reg_names(&self) -> Result<Vec<(RegisterMode, String)>, err::ErrKind> {
        let mut names: Vec<_> = self
            .asm_fmt
            .all_reg_names()?
            .into_iter()
            .map(|(register, name)| (register, name))
            .collect();
        for index in 0..self.asm_fmt.reg_count() {
            names.push((
                RegisterMode::Float(index),
                self.asm_fmt.get_fmt_float_reg(index, &Size::F32)?,
            ));
            names.push((
                RegisterMode::Float(index),
                self.asm_fmt.get_fmt_float_reg(index, &Size::F64)?,
            ));
        }
        Ok(names)
    }

    /// インラインアセンブラに直接書かれたレジスタを検出する。
    fn detect_literal_regs(&self, template: &str) -> Result<Vec<RegisterMode>, err::ErrKind> {
        let mut found = Vec::new();
        for (reg, name) in self.all_inline_reg_names()? {
            let mut from = 0;
            while let Some(offset) = template[from..].find(&name) {
                let end = from + offset + name.len();
                let has_register_suffix = template[end..]
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric());
                if !has_register_suffix {
                    if !found.contains(&reg) {
                        found.push(reg);
                    }
                    break;
                }
                from = end;
            }
        }
        Ok(found)
    }

    /// `exclude`(インラインアセンブラ自身が使うレジスタ)を除いた上で、
    /// 現在使われていない、空いているレジスタ番号を1つ探す。
    ///
    /// `%rax`/`%rdx`(`RESERVED_REGS`)と、まだ定義されていない変数のために
    /// 取り置かれているレジスタは選ばない。
    fn find_free_reg(&self, mode: RegisterMode, exclude: &[RegisterMode]) -> Option<RegisterMode> {
        (0..self.asm_fmt.reg_count())
            .map(|index| match mode {
                RegisterMode::Normal(_) => RegisterMode::Normal(index),
                RegisterMode::Float(_) => RegisterMode::Float(index),
            })
            .find(|reg| {
                !matches!(reg, RegisterMode::Normal(index) if RESERVED_REGS.contains(index))
                    && !exclude.contains(reg)
                    && !self.used_reg.is_used(reg)
                    && !self.used_reg.is_reserved(*reg)
            })
    }

    /// インラインアセンブラが暗黙に書き換えるレジスタ
    /// (`syscall`は`%rcx`と`%r11`を書き換える)
    fn implicit_clobbered_regs(&self, lines: &[(String, Vec<usize>)]) -> Vec<RegisterMode> {
        if !lines
            .iter()
            .any(|(template, _)| template.contains("syscall"))
        {
            return Vec::new();
        }
        [1, 11].into_iter().map(RegisterMode::Normal).collect()
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
    fn inline_ptr_param_operand(&self, node_idx: usize) -> Result<Option<String>, err::ErrKind> {
        let inst::Inst::Param(param) = &self.curr_inst[node_idx] else {
            return Ok(None);
        };
        let Some(var_info) = self.find_var_info(&param.name) else {
            return Ok(None);
        };
        if var_info.size.is_pointer().is_none() {
            return Ok(None);
        }

        self.ptr_deref_operand(var_info.reg).map(Some)
    }

    fn extract_operand_text_sized(
        &mut self,
        node_idx: usize,
        this_is_self: SelfPtrInfo,
        forced_size: &Size,
    ) -> Result<String, err::ErrKind> {
        match self.inst_at(node_idx) {
            inst::Inst::AssignVar { name, .. } => {
                let var_info = self.get_var_all_info(&name)?;
                if var_info.is_stack {
                    // レジスタを持たない変数は、`%rbp`相対のメモリ参照にする
                    return self.var_operand(&name);
                }
                let size = gen_reg_size(&var_info, &forced_size);
                self.reg_mode_operand(var_info.reg, &size)
            }
            inst::Inst::Param(param) => {
                let var_info = self.get_var_all_info(&param.name)?;
                Ok(if var_info.size.is_pointer().is_some() {
                    self.ptr_deref_operand(var_info.reg)?
                } else {
                    self.reg_mode_operand(var_info.reg, forced_size)?
                })
            }
            // それ以外(メモリ参照/即値/ポインタなど)は、サイズの
            // 上書きを行わず通常通り組み立てる
            _ => self.extract_operand_text(node_idx, this_is_self),
        }
    }

    pub(super) fn deploy_inline_asm<R: ResultRet>(
        &mut self,
        name: &str,
        lines: &[(String, Vec<usize>)],
    ) -> Result<(), err::ErrKind> {
        if self
            .asm_fmt
            .inline_asm_list()
            .iter()
            .find(|v| v.as_str() == name)
            .is_some()
        {
            // === 1. インラインアセンブラ中で使われているレジスタを検出する ===
            let mut literal_regs = Vec::<RegisterMode>::new();
            for (template, _) in lines {
                for reg in self.detect_literal_regs(template)? {
                    if !literal_regs.contains(&reg) {
                        literal_regs.push(reg);
                    }
                }
            }
            // `syscall`などが暗黙に書き換えるレジスタも、使われているものとして扱う
            for reg in self.implicit_clobbered_regs(lines) {
                if !literal_regs.contains(&reg) {
                    literal_regs.push(reg);
                }
            }

            // === 2. 使用中のレジスタは、空いている別のレジスタへ移す ===
            // (`push`では、`${msg}`などの埋め込み先が書き換えられたレジスタの
            //  ままになってしまう。移した先のレジスタを`${..}`に埋め込む)
            // 移した (元のレジスタ, 移した先のレジスタ)
            let mut moved_regs: Vec<(RegisterMode, RegisterMode)> = Vec::new();
            // 空きレジスタが無く、スタックへ退避した(後で必ずpopする)レジスタ
            let mut stacked_regs: Vec<RegisterMode> = Vec::new();
            for reg in &literal_regs {
                if self.used_reg.is_used(reg) {
                    match self.mov_register_val_to_reg(*reg, &literal_regs)? {
                        Some(free_reg) => moved_regs.push((*reg, free_reg)),
                        None => self.mov_register_val_to_stack(*reg, &mut stacked_regs)?,
                    }
                }
            }

            // === インラインアセンブラ本体を展開する ===
            self.gen_inline_asm_txt(&lines)?;

            // === 3. 移したレジスタの値を、元のレジスタへ戻す ===
            // (インラインアセンブラが元のレジスタを別の値で書き換えているので、
            //  この後のコードのために変数の位置を元に戻す)
            moved_regs.iter().rev().for_each(|(reg, free_reg)| {
                let binding = self.restore_register_val(*reg, *free_reg);
                R::handle(&self.err_pool, binding);
            });
            // === スタックに退避したレジスタを、退避した時とは逆順に戻す ===
            for reg in stacked_regs.iter().rev() {
                let binding = self.restore_register_val_from_stack(*reg);
                R::handle(&self.err_pool, binding)?;
            }
            Ok(())
        } else {
            Err(crate::GenUndefErrResult!(
                UndefInlineAsm,
                name.to_string(),
                None
            ))
        }
    }

    /// レジスタ`reg`の値(変数・式の一時的な結果)を、インラインアセンブラが
    /// 使わない空いているレジスタへ`mov`し、`reg`を指していた変数と式の結果の
    /// 参照先を、移した先のレジスタに切り替える。
    ///
    /// 空いているレジスタが無ければ何もせず`None`を返す。
    #[inline(always)]
    fn mov_register_val_to_reg(
        &mut self,
        reg: RegisterMode,
        asm_regs: &[RegisterMode],
    ) -> Result<Option<RegisterMode>, err::ErrKind> {
        // 空いているレジスタが無ければ何もしない
        let Some(free_reg) = self.find_free_reg(reg, asm_regs) else {
            return Ok(None);
        };

        let mov_asm = self.register_move(&free_reg, &reg)?;
        self.asm_text.push_str(&mov_asm);

        self.retarget_reg(reg, free_reg);
        Ok(Some(free_reg))
    }

    /// `mov_register_val_to_reg`で`free_reg`へ移した値を、元の`reg`へ戻す
    #[inline(always)]
    fn restore_register_val(
        &mut self,
        reg: RegisterMode,
        free_reg: RegisterMode,
    ) -> Result<(), err::ErrKind> {
        let mov_asm = self.register_move(&reg, &free_reg)?;
        self.asm_text.push_str(&mov_asm);

        self.retarget_reg(free_reg, reg);
        // 一時的に使った`free_reg`を解放する
        self.used_reg.release(&free_reg);
        Ok(())
    }

    /// レジスタ`from`を保持している変数と式の結果を、`to`を保持するように切り替える
    fn retarget_reg(&mut self, from: RegisterMode, to: RegisterMode) {
        let names: Vec<String> = self
            .var_hash_map
            .iter()
            .filter(|(_, var)| !var.is_stack && var.reg == from)
            .map(|(name, _)| name.clone())
            .collect();
        names
            .iter()
            .for_each(|name| self.update_value_reg(name, &to));
        self.last_inst_idx
            .iter_mut()
            .filter(|(_, reg)| *reg == from)
            .for_each(|(_, reg)| *reg = to);
        self.used_reg.mark_used(&to);
    }

    /// レジスタが空いていない場合に、スタックに退避する
    #[inline(always)]
    fn mov_register_val_to_stack(
        &mut self,
        reg: RegisterMode,
        stacked_regs: &mut Vec<RegisterMode>,
    ) -> Result<(), err::ErrKind> {
        match reg {
            RegisterMode::Normal(_) => {
                let reg_name = self.reg64(&reg)?;
                let push_asm = self.asm_fmt.get_push(&reg_name);
                self.asm_text.push_str(&push_asm);
            }
            RegisterMode::Float(_) => {
                self.asm_text.push_str("sub $16, %rsp\n");
                let size = self.float_reg_size(reg);
                let opcode = Self::float_move_opcode(&size);
                let line = self
                    .asm_fmt
                    .get_opcode_tmpl(opcode)
                    .replace("{src1}", &self.reg_mode_operand(reg, &size)?)
                    .replace("{dst}", "(%rsp)");
                self.asm_text.push_str(&line);
            }
        }
        stacked_regs.push(reg);
        Ok(())
    }

    fn restore_register_val_from_stack(&mut self, reg: RegisterMode) -> Result<(), err::ErrKind> {
        match reg {
            RegisterMode::Normal(_) => {
                let pop_asm = self.asm_fmt.get_pop(&self.reg64(&reg)?);
                self.asm_text.push_str(&pop_asm);
            }
            RegisterMode::Float(_) => {
                let size = self.float_reg_size(reg);
                let opcode = Self::float_move_opcode(&size);
                let line = self
                    .asm_fmt
                    .get_opcode_tmpl(opcode)
                    .replace("{src1}", "(%rsp)")
                    .replace("{dst}", &self.reg_mode_operand(reg, &size)?);
                self.asm_text.push_str(&line);
                self.asm_text.push_str("add $16, %rsp\n");
            }
        }
        Ok(())
    }

    fn register_move(
        &self,
        dst: &RegisterMode,
        src: &RegisterMode,
    ) -> Result<String, err::ErrKind> {
        Ok(match (dst, src) {
            (RegisterMode::Normal(_), RegisterMode::Normal(_)) => {
                self.mov_line(&self.reg64(dst)?, &self.reg64(src)?, &Size::DQ, false)
            }
            (RegisterMode::Float(_), RegisterMode::Float(_)) => {
                let size = self.float_reg_size(*src);
                let opcode = Self::float_move_opcode(&size);
                self.asm_fmt
                    .get_opcode_tmpl(opcode)
                    .replace("{src1}", &self.reg_mode_operand(*src, &size)?)
                    .replace("{dst}", &self.reg_mode_operand(*dst, &size)?)
            }
            _ => panic!("cannot move between normal and floating-point registers"),
        })
    }

    pub(super) fn float_reg_size(&self, reg: RegisterMode) -> Size {
        let from_var = self
            .var_hash_map
            .values()
            .find(|var| !var.is_stack && var.reg == reg)
            .map(|var| var.size.clone());
        let from_expr = self
            .last_inst_idx
            .iter()
            .find(|(_, result_reg)| *result_reg == reg)
            .map(|(node_idx, _)| self.get_expr_ty(*node_idx));
        from_var
            .or(from_expr)
            .filter(|size| size.is_float())
            .unwrap_or(Size::F64)
    }

    pub(super) fn float_move_opcode(size: &Size) -> &'static str {
        match size {
            Size::F32 => "float_mov",
            Size::F64 => "f64_mov",
            _ => panic!(
                "floating-point register has non-floating-point type: {:?}",
                size
            ),
        }
    }

    pub(super) fn reg_mode_operand(
        &self,
        reg: RegisterMode,
        size: &Size,
    ) -> Result<String, err::ErrKind> {
        match reg {
            RegisterMode::Normal(index) => self.asm_fmt.get_fmt_reg(index, size),
            RegisterMode::Float(index) => self.asm_fmt.get_fmt_float_reg(index, size),
        }
    }

    #[inline(always)]
    fn gen_inline_asm_txt(&mut self, lines: &[(String, Vec<usize>)]) -> Result<(), err::ErrKind> {
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
                    self.inline_ptr_param_operand(*operand_id)?
                {
                    ptr_operand
                } else {
                    match (index != last_index, &dst_size) {
                        (true, Some(size)) => {
                            self.extract_operand_text_sized(*operand_id, dst_size.as_ref(), size)?
                        }
                        _ => self.extract_operand_text(*operand_id, dst_size.as_ref())?,
                    }
                };
                // `{0}`, `{1}`, ... という数字のプレースホルダーを置換
                // (`${var}`はパーサー側(preproc.rs)の時点で既に
                //  `{index}`へ変換済みのため、ここでは数字の
                //  プレースホルダーだけを見れば良い)
                asm_line = asm_line.replace(&format!("{{{}}}", index), &operand_text);
            }

            // ユーザーが書いた行なので、サイズ接尾辞の自動付与の対象外にする
            asm_line.push_str(RAW_ASM_MARK);
            asm_line.push('\n');
            self.asm_text.push_str(&asm_line);
        }
        Ok(())
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
