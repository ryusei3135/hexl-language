//! 命令 (ニーモニック) と、その機械語を生成する処理の対応表
//!
//! # 命令を追加するには (このファイルだけ編集すればよい)
//!
//! 1. 下の `dispatch` の `match` に1行足す。
//!
//!    ```text
//!    "nop" => e.emit_nop(&inst.operands),
//!    ```
//!
//!    サフィックス違い (`addq` / `addl` ...) は `|` で並べて同じ行に書く。
//!    条件付きジャンプのように追加の引数が要る場合は、ここで渡す
//!    (例: `"jl" => e.emit_jcc(Condition::L, &inst.operands)`)。
//!
//! 2. 呼び出す `emit_xxx` を、このファイル下部の `impl Emitter` に書く
//!    (`emit_nop` が最小の見本)。`emit_u8` / `emit_rex` /
//!    `emit_memory_modrm` など、`emitter.rs` の非公開ヘルパーもそのまま使える。
//!
//! 未対応のニーモニックは `unsupported instruction: ...` のエラーになる。

use super::*;

/// mov系ニーモニックのサフィックスからオペランドサイズ(bit数)を求める。
/// レジスタが絡む場合はレジスタ名からサイズが分かるため使われないが、
/// `movl $10, -4(%rbp)` のようにメモリ+即値の組み合わせでは、
/// サイズを判定できる手がかりがサフィックスしかない。
fn mov_size_bits(mnemonic: &str) -> u32 {
    match mnemonic {
        "movb" => 8,
        "movw" => 16,
        "movq" => 64,
        _ => 32, // "mov", "movl" 及びその他はデフォルトで32bitとする
    }
}

/// neg系ニーモニックのサフィックスからオペランドサイズ(bit数)を求める。
/// サフィックス無しの `neg` は `None` を返す。
/// レジスタ操作ならレジスタ名からサイズが決まるが、`negl -4(%rbp)` のような
/// メモリ操作ではサフィックスだけが手がかりになる。
fn neg_suffix_bits(mnemonic: &str) -> Option<u32> {
    match mnemonic {
        "negb" => Some(8),
        "negw" => Some(16),
        "negl" => Some(32),
        "negq" => Some(64),
        _ => None,
    }
}

/// ニーモニックに対応する処理を呼び出す
pub(super) fn dispatch(e: &mut Emitter, inst: &Instruction) -> Result<(), String> {
    let name = inst.mnemonic.as_str();
    match name {
        "push" | "pushq" | "pushl" | "pushw" => e.emit_push(&inst.operands),

        "pop" | "popq" | "popl" | "popw" => e.emit_pop(&inst.operands),

        "mov" | "movq" | "movl" | "movw" | "movb" => {
            e.emit_mov(&inst.operands, mov_size_bits(name))
        }

        "add" | "addq" | "addl" | "addw" | "addb" => e.emit_add(&inst.operands),

        "sub" | "subq" | "subl" | "subw" | "subb" => e.emit_sub(&inst.operands),

        "imul" | "imulq" | "imull" | "imulw" => e.emit_imul(&inst.operands),

        "mul" | "mulq" | "mull" | "mulw" => e.emit_mul(&inst.operands),

        "div" | "divq" | "divl" | "divw" => e.emit_div(&inst.operands),

        "idiv" | "idivq" | "idivl" | "idivw" => e.emit_idiv(&inst.operands),

        "cltd" | "cdq" => e.emit_cltd(),

        "cqto" | "cqo" => e.emit_cqto(&inst.operands),

        "neg" | "negq" | "negl" | "negw" | "negb" => {
            e.emit_neg(&inst.operands, neg_suffix_bits(name))
        }

        "movslq" => e.emit_movslq(&inst.operands),

        "cmp" | "cmpq" | "cmpl" | "cmpw" | "cmpb" => e.emit_cmp(&inst.operands),

        "lea" | "leaq" | "leal" => e.emit_lea(&inst.operands),

        "jl" => e.emit_jcc(Condition::L, &inst.operands),

        "jle" => e.emit_jcc(Condition::LE, &inst.operands),

        "jg" => e.emit_jcc(Condition::G, &inst.operands),

        "jge" => e.emit_jcc(Condition::GE, &inst.operands),

        "jz" | "je" => e.emit_jcc(Condition::E, &inst.operands),

        "jnz" | "jne" => e.emit_jcc(Condition::NE, &inst.operands),

        "jmp" => e.emit_jmp(&inst.operands),

        "call" => e.emit_call(&inst.operands),

        "ret" => e.emit_ret(&inst.operands),

        "leave" => e.emit_leave(&inst.operands),

        "syscall" => e.emit_syscall(&inst.operands),

        "nop" => e.emit_nop(&inst.operands),

        _ => Err(format!("unsupported instruction: {}", name)),
    }
}

// ============================================================
// このファイルで定義する命令
// (既存の命令は `emitter.rs` にある。新しい命令はここに足す)
// ============================================================

impl Emitter {
    // --------------------------------------------------------
    // nop  (0x90)  ← 命令追加の見本
    // --------------------------------------------------------
    fn emit_nop(&mut self, operands: &[Operand]) -> Result<(), String> {
        if !operands.is_empty() {
            return Err("nop takes no operands".into());
        }
        self.emit_u8(0x90);

        Ok(())
    }

    // --------------------------------------------------------
    // cqto / cqo  (REX.W 0x99)
    //
    // `%rax`の符号ビットを`%rdx`全体へ拡張する(`%rdx:%rax`を作る)。
    // 64bitの`idiv`の前に使う。32bit版の`cltd`/`cdq`(0x99)に
    // REX.Wプレフィックスを付けたもの
    // --------------------------------------------------------
    fn emit_cqto(&mut self, operands: &[Operand]) -> Result<(), String> {
        if !operands.is_empty() {
            return Err("cqto takes no operands".into());
        }
        self.emit_rex(true, false, false, false);
        self.emit_u8(0x99);

        Ok(())
    }

    // --------------------------------------------------------
    // neg  (F6 /3, F7 /3)
    //
    // 2の補数による符号反転 (オペランドを `0 - オペランド` にする)。
    //
    //   negq %rax        -> REX.W F7 /3
    //   negl -4(%rbp)    -> F7 /3
    //   negb %al         -> F6 /3
    //
    // - レジスタ: サイズはレジスタ名で決まる。サフィックスが付いていて
    //   レジスタ幅と食い違う場合はエラー。
    // - メモリ  : サイズはサフィックスで決まる。無印 `neg` は `mov` と
    //   同じく32bit扱い。
    // --------------------------------------------------------
    fn emit_neg(&mut self, operands: &[Operand], suffix_bits: Option<u32>) -> Result<(), String> {
        if operands.len() != 1 {
            return Err("neg requires one operand".into());
        }

        match &operands[0] {
            Operand::Register(reg) => {
                let bits = reg.bits() as u32;

                if let Some(suffix) = suffix_bits {
                    if suffix != bits {
                        return Err(format!(
                            "neg: operand size suffix ({} bit) does not match register size ({} bit)",
                            suffix, bits
                        ));
                    }
                }

                if bits == 16 {
                    self.emit_u8(0x66);
                }

                let extended = reg.id() >= 8;

                if bits == 8 && !extended && (4..=7).contains(&reg.id()) {
                    // %spl / %bpl / %sil / %dil は REX プレフィックスが
                    // 無いと %ah / %ch / %dh / %bh の意味になってしまうため、
                    // 中身が空の REX (0x40) を必ず付ける。
                    self.emit_u8(0x40);
                } else {
                    self.emit_rex(bits == 64, false, false, extended);
                }

                self.emit_u8(if bits == 8 { 0xF6 } else { 0xF7 });
                self.emit_u8(0b11_000_000 | (3 << 3) | (reg.id() & 7));

                Ok(())
            }

            Operand::Memory(mem) => {
                let bits = suffix_bits.unwrap_or(32);

                if bits == 16 {
                    self.emit_u8(0x66);
                }

                self.emit_rex(
                    bits == 64,
                    false,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                );

                self.emit_u8(if bits == 8 { 0xF6 } else { 0xF7 });

                self.emit_memory_modrm(3, mem)
            }

            _ => Err("invalid neg operand".into()),
        }
    }

    // --------------------------------------------------------
    // movslq  (REX.W 63 /r)  -- Intel表記の movsxd r64, r/m32
    //
    // 32bitの値を符号拡張して64bitレジスタへ書き込む。
    //
    //   movslq %eax, %rax
    //   movslq -4(%rbp), %rdx
    //
    // ModRM の reg が宛先 (64bitレジスタ)、rm が元 (32bitレジスタ/メモリ)。
    // --------------------------------------------------------
    fn emit_movslq(&mut self, operands: &[Operand]) -> Result<(), String> {
        if operands.len() != 2 {
            return Err("movslq requires two operands".into());
        }

        let dst = match &operands[1] {
            Operand::Register(reg) if reg.bits() == 64 => *reg,
            _ => return Err("movslq destination must be a 64-bit register".into()),
        };

        match &operands[0] {
            Operand::Register(src) => {
                if src.bits() != 32 {
                    return Err("movslq source register must be 32-bit".into());
                }

                self.emit_rex(true, dst.id() >= 8, false, src.id() >= 8);
                self.emit_u8(0x63);
                self.emit_u8(0b11_000_000 | ((dst.id() & 7) << 3) | (src.id() & 7));

                Ok(())
            }

            Operand::Memory(mem) => {
                self.emit_rex(
                    true,
                    dst.id() >= 8,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                );
                self.emit_u8(0x63);

                self.emit_memory_modrm(dst.id(), mem)
            }

            _ => Err("invalid movslq source operand".into()),
        }
    }
}