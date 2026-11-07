use super::*;

impl AsmEmitter {
    /// `regs`のうちcallerが保存するレジスタだけを`push`するアセンブリを返す
    /// (`callee_saved`のレジスタは呼ばれた側が保存するので退避しない)
    pub(super) fn is_callee_saved(&self, reg: RegisterMode) -> bool {
        match reg {
            RegisterMode::Normal(index) => self.callee_saved.contains(&index),
            RegisterMode::Float(index) => self.float_callee_saved.contains(&index),
        }
    }

    /// 1つのレジスタを退避する命令(`push`、浮動小数点数は16byteの領域へ`mov`)と、
    /// そのレジスタが使うスタックのバイト数
    fn caller_save_one(&self, reg: RegisterMode) -> Result<(String, usize), err::ErrKind> {
        Ok(match reg {
            RegisterMode::Normal(index) => {
                let reg_name = self.asm_fmt.get_fmt_reg(index, &Size::DQ)?;
                (self.asm_fmt.get_push(&reg_name), 8)
            }
            RegisterMode::Float(_) => {
                let size = self.float_reg_size(reg);
                let opcode = Self::float_move_opcode(&size);
                let operand = self.reg_mode_operand(reg, &size)?;
                (
                    format!(
                        "{}{}",
                        self.asm_fmt.get_stack_alloc_16(),
                        self.asm_fmt
                            .get_opcode_tmpl(opcode)
                            .replace("{src1}", &operand)
                            .replace("{dst}", &self.asm_fmt.get_stack_top_ref())
                    ),
                    16,
                )
            }
        })
    }

    /// `caller_save_one`で退避したレジスタ1つを復元する命令
    fn caller_restore_one(&self, reg: RegisterMode) -> Result<String, err::ErrKind> {
        Ok(match reg {
            RegisterMode::Normal(index) => {
                let reg_name = self.asm_fmt.get_fmt_reg(index, &Size::DQ)?;
                self.asm_fmt.get_pop(&reg_name)
            }
            RegisterMode::Float(_) => {
                let size = self.float_reg_size(reg);
                let opcode = Self::float_move_opcode(&size);
                let operand = self.reg_mode_operand(reg, &size)?;
                format!(
                    "{}{}",
                    self.asm_fmt
                        .get_opcode_tmpl(opcode)
                        .replace("{src1}", &self.asm_fmt.get_stack_top_ref())
                        .replace("{dst}", &operand),
                    self.asm_fmt.get_stack_free_16()
                )
            }
        })
    }

    /// 関数呼び出しの前後で`regs`(callerが保存するレジスタ)を退避/復元する
    /// 命令の組`(退避, 復元)`を、同じ`regs`から同時に作る
    ///
    /// - 退避は`regs`の順、復元はその逆順なので、`push`と`pop`が必ず対応する
    /// - 退避したバイト数が16の倍数でなければ、退避の最後に8byteの余白を確保し、
    ///   復元の最初に解放する。これで`call`の時点の`%rsp`が16byte境界になる
    ///   (関数の入口側で、`callee_saved`の退避を含めてフレームを16byte境界に
    ///   揃えている。`fix_callee_saved`を参照)
    pub(super) fn caller_save_pair(
        &self,
        regs: &[RegisterMode],
    ) -> Result<(String, String), err::ErrKind> {
        let regs: Vec<RegisterMode> = regs
            .iter()
            .copied()
            .filter(|reg| !self.is_callee_saved(*reg))
            .collect();
        let mut save = String::new();
        let mut restore = String::new();
        let mut bytes = 0usize;
        for reg in &regs {
            let (asm, used) = self.caller_save_one(*reg)?;
            save.push_str(&asm);
            bytes += used;
        }
        if bytes % 16 != 0 {
            save.push_str(&self.asm_fmt.get_stack_alloc_8());
            restore.push_str(&self.asm_fmt.get_stack_free_8());
        }
        for reg in regs.iter().rev() {
            restore.push_str(&self.caller_restore_one(*reg)?);
        }
        Ok((save, restore))
    }
}
