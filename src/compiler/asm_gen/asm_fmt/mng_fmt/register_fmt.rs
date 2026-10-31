//! レジスタのフォーマットを返すモジュール

use super::*;

impl MngAsmFmt {

    /// オペランドの並びから、最後のレジスタオペランドの幅に対応する接尾辞を返す
    /// (`(%rbp)`のようなメモリ参照の中のレジスタは数えない)
    pub(in super) fn last_reg_suffix(operands: &str) -> Option<char> {
        let mut depth = 0i32;
        let mut parts = Vec::<&str>::new();
        let mut start = 0;
        for (i, c) in operands.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(&operands[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        parts.push(&operands[start..]);

        parts.iter().rev().find_map(|p| {
            let name = p.trim().strip_prefix('%')?;
            if name.contains('(') || name.starts_with("xmm") || name.starts_with("ymm") {
                return None;
            }
            Self::reg_width_suffix(name)
        })
    }

    pub(in super) fn reg_width_suffix(name: &str) -> Option<char> {
        const Q: &[&str] = &["rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp"];
        const L: &[&str] = &["eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp"];
        const W: &[&str] = &["ax", "bx", "cx", "dx", "si", "di", "bp", "sp"];
        const B: &[&str] = &[
            "al", "bl", "cl", "dl", "sil", "dil", "bpl", "spl", "ah", "bh", "ch", "dh",
        ];
        if Q.contains(&name) {
            return Some('q');
        }
        if L.contains(&name) {
            return Some('l');
        }
        if W.contains(&name) {
            return Some('w');
        }
        if B.contains(&name) {
            return Some('b');
        }
        // r8 - r15 (+ d/w/b)
        let rest = name.strip_prefix('r')?;
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let n: u32 = digits.parse().ok()?;
        if !(8..=15).contains(&n) {
            return None;
        }
        match &rest[digits.len()..] {
            "" => Some('q'),
            "d" => Some('l'),
            "w" => Some('w'),
            "b" => Some('b'),
            _ => None,
        }
    }

    pub(in crate::compiler::asm_gen) fn get_fmt_reg(
        &self,
        reg_num: usize,
        size: &Size,
    ) -> Result<String, err::ErrKind> {
        let reg = match &size {
            Size::DB | Size::UDB => &self.reg_fmt.db,
            Size::DW | Size::UDW => &self.reg_fmt.dw,
            Size::DD | Size::UDD => &self.reg_fmt.dd,
            Size::DQ | Size::UDQ => &self.reg_fmt.dq,
            Size::Pointer { .. } => &self.reg_fmt.dq,
            t => {
                crate::GenCompilerErr!(CompilerErrKind::ThisTyIsCantConvertRegsiter(size.clone()))?
            }
        };
        Ok(self
            .fmt
            .fmt
            .reg
            .replace("{}", reg[reg_num].as_str())
            .to_string())
    }

    pub(in crate::compiler::asm_gen) fn get_fmt_float_reg(
        &self,
        reg_num: usize,
        size: &Size,
    ) -> Result<String, err::ErrKind> {
        let reg = match &size {
            Size::F32 => &self.reg_fmt.xmm,
            Size::F64 => &self.reg_fmt.xmm,
            Size::Pointer { .. } => &self.reg_fmt.dq,
            _ => {
                crate::GenCompilerErr!(CompilerErrKind::ThisTyIsCantConvertRegsiter(size.clone()))?
            }
        };
        Ok(self
            .fmt
            .fmt
            .reg
            .replace("{}", reg[reg_num].as_str())
            .to_string())
    }

    /// 確保されているレジスタの本数
    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn reg_count(&self) -> usize {
        self.reg_fmt.dq.len()
    }

    pub(in crate::compiler::asm_gen) fn float_reg_count(&self) -> usize {
        self.reg_fmt.xmm.len()
    }

    /// `(レジスタ番号, レジスタ名)`のペアを、確保されている
    /// 全レジスタ・全サイズ分列挙する。
    pub(in crate::compiler::asm_gen) fn all_reg_names(
        &self,
    ) -> Result<Vec<(RegisterMode, String)>, err::ErrKind> {
        let mut result = Vec::new();
        for reg_idx in 0..self.reg_count() {
            for size in [Size::DQ, Size::DD, Size::DW, Size::DB] {
                result.push((
                    RegisterMode::Normal(reg_idx),
                    self.get_fmt_reg(reg_idx, &size)?,
                ));
            }
        }
        for reg_idx in 0..self.float_reg_count() {
            for size in [Size::F32, Size::F64] {
                result.push((
                    RegisterMode::Float(reg_idx),
                    self.get_fmt_float_reg(reg_idx, &size)?,
                ));
            }
        }
        Ok(result)
    }
    
    /// 静的領域の変数に%ripをつけて返す
    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn fmt_static_var_rip(&self, name: &str) -> String {
        self.fmt.fmt.static_var.replace("{name}", name)
    }

    /// 先頭から`count`個の引数が渡されるレジスタの番号
    pub(in crate::compiler::asm_gen) fn param_regs(&self, count: usize) -> Vec<usize> {
        self.param_fmt.iter().take(count).copied().collect()
    }

    /// 渡された文字列がレジスタのオペランド(`%eax`など)である場合、
    /// 「同じレジスタ」の`size`版のオペランド文字列(`%rax`など)に
    /// 変換して返す。
    ///
    /// `operand`がレジスタでない場合(即値やメモリ参照`-8(%rbp)`など)
    /// は、`get_reg_size`が`None`を返すのでそのまま変更せず返す。
    pub(in crate::compiler::asm_gen) fn resize_reg_operand(
        &self,
        operand: &str,
        size: &Size,
    ) -> Result<String, err::ErrKind> {
        for (_, registers) in [
            (Size::DB, &self.reg_fmt.db),
            (Size::DW, &self.reg_fmt.dw),
            (Size::DD, &self.reg_fmt.dd),
            (Size::DQ, &self.reg_fmt.dq),
        ] {
            for (reg_num, register) in registers.iter().enumerate() {
                let formatted = self.fmt.fmt.reg.replace("{}", register);
                if formatted == operand {
                    return self.get_fmt_reg(reg_num, size);
                }
            }
        }
        Ok(operand.to_string())
    }
}