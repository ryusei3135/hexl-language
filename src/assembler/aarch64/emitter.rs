//! aarch64 のエミッタ本体
//!
//! 構文解析済みのプログラム (`parse::Program`) を受け取り、
//! `.text` / `.data` のバイト列とラベル表、未解決の参照 (`Fixup`) を作る。
//!
//! - 命令の符号化は `emitter/inst_table.rs`
//! - ディレクティブの処理は `directives.rs`
//! - 出力ファイル (ELF) の組み立ては `elf.rs`
//!
//! aarch64 の命令はすべて 32bit 固定長で、リトルエンディアンで書き出す。

use std::collections::HashMap;

use super::directives;
use super::parse::{Program, Statement};

mod inst_table;

// ============================================================
// Section
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Text,
    Data,
}

// ============================================================
// Relocation / Fixup
// ============================================================

/// 未解決の参照の種類。
///
/// 新しい種類の再配置を足すときは、下の `elf_reloc_type` と `patch` (と必要なら
/// `is_pc_relative`) に1行ずつ足す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixupKind {
    /// `b label` (imm26, ±128MiB)
    Jump26,
    /// `bl label` (imm26, ±128MiB)
    Call26,
    /// `b.cond` / `cbz` / `cbnz` (imm19, ±1MiB)
    CondBr19,
    /// `tbz` / `tbnz` (imm14, ±32KiB)
    TstBr14,
    /// `ldr x0, label` (imm19, ±1MiB)
    LdrLit19,
    /// `adr` (21bit, ±1MiB)
    AdrLo21,
    /// `adrp` (ページ単位 21bit, ±4GiB)
    AdrpPage21,
    /// `add x0, x0, :lo12:sym`
    AddLo12,
    /// `ldr x0, [x1, :lo12:sym]` (値はアクセスサイズの log2: 0=byte, 1=half, 2=word, 3=dword)
    LdstLo12(u8),
    /// `.word sym`
    Abs32,
    /// `.quad sym`
    Abs64,
}

impl FixupKind {
    /// ELF の再配置タイプ番号 (`Elf64_Rela.r_info` の下位32bit)
    pub fn elf_reloc_type(self) -> u32 {
        match self {
            FixupKind::Abs64 => 257,       // R_AARCH64_ABS64
            FixupKind::Abs32 => 258,       // R_AARCH64_ABS32
            FixupKind::LdrLit19 => 273,    // R_AARCH64_LD_PREL_LO19
            FixupKind::AdrLo21 => 274,     // R_AARCH64_ADR_PREL_LO21
            FixupKind::AdrpPage21 => 275,  // R_AARCH64_ADR_PREL_PG_HI21
            FixupKind::AddLo12 => 277,     // R_AARCH64_ADD_ABS_LO12_NC
            FixupKind::LdstLo12(0) => 278, // R_AARCH64_LDST8_ABS_LO12_NC
            FixupKind::LdstLo12(1) => 284, // R_AARCH64_LDST16_ABS_LO12_NC
            FixupKind::LdstLo12(2) => 285, // R_AARCH64_LDST32_ABS_LO12_NC
            FixupKind::LdstLo12(_) => 286, // R_AARCH64_LDST64_ABS_LO12_NC
            FixupKind::TstBr14 => 279,     // R_AARCH64_TSTBR14
            FixupKind::CondBr19 => 280,    // R_AARCH64_CONDBR19
            FixupKind::Jump26 => 282,      // R_AARCH64_JUMP26
            FixupKind::Call26 => 283,      // R_AARCH64_CALL26
        }
    }

    /// 同じセクション内なら、配置が決まる前に (位置の差だけで) 解決できる種類か
    pub fn is_pc_relative(self) -> bool {
        matches!(
            self,
            FixupKind::Jump26
                | FixupKind::Call26
                | FixupKind::CondBr19
                | FixupKind::TstBr14
                | FixupKind::LdrLit19
                | FixupKind::AdrLo21
        )
    }

    /// `buf[offset..]` の命令 / データへ、シンボルのアドレス `s` と
    /// 参照位置のアドレス `p` から計算した値を埋め込む。
    ///
    /// PC相対の種類は `s - p` だけを使うので、`s` と `p` を同じ基準
    /// (セクション先頭からのオフセットでも絶対アドレスでも) で渡せばよい。
    pub fn patch(self, buf: &mut [u8], offset: usize, s: u64, p: u64) -> Result<(), String> {
        let pc_off = s.wrapping_sub(p) as i64;

        let get =
            |buf: &[u8]| -> u32 { u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap()) };

        let put = |buf: &mut [u8], word: u32| {
            buf[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
        };

        let check_branch = |bits: u32, what: &str| -> Result<i64, String> {
            if pc_off % 4 != 0 {
                return Err(format!("{}: ターゲットが4バイト境界にありません", what));
            }

            let limit = 1i64 << (bits + 1); // 命令数 (2^(bits-1)) × 4 バイト
            if pc_off < -limit || pc_off >= limit {
                return Err(format!("{}: 分岐先が範囲外です", what));
            }

            Ok(pc_off >> 2)
        };

        match self {
            FixupKind::Jump26 | FixupKind::Call26 => {
                let imm = check_branch(26, "b/bl")?;
                put(buf, get(buf) | (imm as u32 & 0x03ff_ffff));
            }

            FixupKind::CondBr19 | FixupKind::LdrLit19 => {
                let imm = check_branch(19, "b.cond/cbz/ldr")?;
                put(buf, get(buf) | ((imm as u32 & 0x7_ffff) << 5));
            }

            FixupKind::TstBr14 => {
                let imm = check_branch(14, "tbz/tbnz")?;
                put(buf, get(buf) | ((imm as u32 & 0x3fff) << 5));
            }

            FixupKind::AdrLo21 => {
                if pc_off < -(1 << 20) || pc_off >= (1 << 20) {
                    return Err("adr: 参照先が範囲外です (±1MiB)".to_string());
                }

                let imm = pc_off as u32;
                put(
                    buf,
                    get(buf) | ((imm & 3) << 29) | (((imm >> 2) & 0x7_ffff) << 5),
                );
            }

            FixupKind::AdrpPage21 => {
                let pages = (s >> 12) as i64 - (p >> 12) as i64;

                if pages < -(1 << 20) || pages >= (1 << 20) {
                    return Err("adrp: 参照先が範囲外です (±4GiB)".to_string());
                }

                let imm = pages as u32;
                put(
                    buf,
                    get(buf) | ((imm & 3) << 29) | (((imm >> 2) & 0x7_ffff) << 5),
                );
            }

            FixupKind::AddLo12 => {
                put(buf, get(buf) | (((s & 0xfff) as u32) << 10));
            }

            FixupKind::LdstLo12(shift) => {
                let lo = (s & 0xfff) as u32;

                if lo % (1 << shift) != 0 {
                    return Err("ldr/str: :lo12: のアドレスがアクセスサイズに揃っていません".into());
                }

                put(buf, get(buf) | ((lo >> shift) << 10));
            }

            FixupKind::Abs32 => {
                if s > u32::MAX as u64 {
                    return Err("アドレスが32bitに収まりません".to_string());
                }

                buf[offset..offset + 4].copy_from_slice(&(s as u32).to_le_bytes());
            }

            FixupKind::Abs64 => {
                buf[offset..offset + 8].copy_from_slice(&s.to_le_bytes());
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Fixup {
    pub section: Section,
    pub offset: usize,
    pub symbol: String,
    pub kind: FixupKind,
}

// ============================================================
// Emitter
// ============================================================

pub struct Emitter {
    pub text: Vec<u8>,
    pub data: Vec<u8>,

    pub current_section: Section,

    pub text_labels: HashMap<String, usize>,
    pub data_labels: HashMap<String, usize>,

    pub globals: Vec<String>,
    pub externs: Vec<String>,

    /// `finish()` の後に残っている未解決の参照
    /// (別セクション / 外部シンボルへの参照と、絶対アドレスが必要なもの)
    pub fixups: Vec<Fixup>,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            text: Vec::new(),
            data: Vec::new(),
            current_section: Section::Text,
            text_labels: HashMap::new(),
            data_labels: HashMap::new(),
            globals: Vec::new(),
            externs: Vec::new(),
            fixups: Vec::new(),
        }
    }

    pub fn emit_program(&mut self, program: &Program) -> Result<(), String> {
        for statement in &program.statements {
            match statement {
                Statement::Label(name) => self.emit_label(name)?,
                Statement::Directive(name, operands) => {
                    directives::emit_directive(self, name, operands)?
                }
                Statement::Instruction(inst) => {
                    // ニーモニック -> 処理 の対応表は `emitter/inst_table.rs`
                    if self.current_section != Section::Text {
                        return Err(format!("instruction `{}` outside .text", inst.mnemonic));
                    }

                    if self.text.len() % 4 != 0 {
                        return Err(format!(
                            "instruction `{}` is not 4-byte aligned",
                            inst.mnemonic
                        ));
                    }

                    inst_table::dispatch(self, inst)
                        .map_err(|e| format!("{}: {}", inst.mnemonic, e))?;
                }
            }
        }

        Ok(())
    }

    /// 同じセクション内で完結する PC 相対の参照を解決し、
    /// 未定義のシンボルを検出する。残りは出力側 (`elf.rs`) が解決する。
    pub fn finish(&mut self) -> Result<(), String> {
        let fixups = std::mem::take(&mut self.fixups);
        let mut remaining = Vec::new();

        for fixup in fixups {
            let defined = self.text_labels.contains_key(&fixup.symbol)
                || self.data_labels.contains_key(&fixup.symbol);

            if !defined && !self.externs.contains(&fixup.symbol) {
                return Err(format!("undefined symbol: {}", fixup.symbol));
            }

            let same_section_target = match fixup.section {
                Section::Text => self.text_labels.get(&fixup.symbol),
                Section::Data => self.data_labels.get(&fixup.symbol),
            };

            if fixup.kind.is_pc_relative() {
                if let Some(&target) = same_section_target {
                    let buf = match fixup.section {
                        Section::Text => &mut self.text,
                        Section::Data => &mut self.data,
                    };

                    fixup
                        .kind
                        .patch(buf, fixup.offset, target as u64, fixup.offset as u64)
                        .map_err(|e| format!("{} ({})", e, fixup.symbol))?;

                    continue;
                }
            }

            remaining.push(fixup);
        }

        self.fixups = remaining;

        Ok(())
    }

    fn emit_label(&mut self, name: &str) -> Result<(), String> {
        let offset = self.current_buffer_len();

        let table = match self.current_section {
            Section::Text => &mut self.text_labels,
            Section::Data => &mut self.data_labels,
        };

        if table.contains_key(name) {
            return Err(format!("duplicate label: {}", name));
        }

        table.insert(name.to_string(), offset);

        Ok(())
    }

    // ========================================================
    // 低レベルの書き込み (inst_table.rs / directives.rs から使う)
    // ========================================================

    pub fn current_buffer(&mut self) -> &mut Vec<u8> {
        match self.current_section {
            Section::Text => &mut self.text,
            Section::Data => &mut self.data,
        }
    }

    pub fn current_buffer_len(&self) -> usize {
        match self.current_section {
            Section::Text => self.text.len(),
            Section::Data => self.data.len(),
        }
    }

    /// 命令を1つ (32bit) 書き出す
    pub fn emit_u32(&mut self, word: u32) {
        self.text.extend_from_slice(&word.to_le_bytes());
    }

    /// 次に書き出す命令のアドレスで未解決の参照を記録する
    /// (`emit_u32` の **前** に呼ぶ)
    pub fn add_fixup(&mut self, symbol: &str, kind: FixupKind) {
        self.fixups.push(Fixup {
            section: self.current_section,
            offset: self.text.len(),
            symbol: symbol.to_string(),
            kind,
        });
    }

    /// データ領域の現在位置で未解決の参照を記録する (`.word sym` / `.quad sym`)
    pub fn add_data_fixup(&mut self, symbol: &str, kind: FixupKind) {
        let offset = self.current_buffer_len();

        self.fixups.push(Fixup {
            section: self.current_section,
            offset,
            symbol: symbol.to_string(),
            kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_range_is_checked() {
        let mut buf = vec![0u8; 4];
        let kind = FixupKind::Jump26;

        // ±128MiB の外
        assert!(kind.patch(&mut buf, 0, 1 << 27, 0).is_err());
        // 4バイト境界でない
        assert!(kind.patch(&mut buf, 0, 2, 0).is_err());
        assert!(kind.patch(&mut buf, 0, 8, 0).is_ok());
    }
}
