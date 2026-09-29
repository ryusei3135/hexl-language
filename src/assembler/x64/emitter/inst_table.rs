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
}
