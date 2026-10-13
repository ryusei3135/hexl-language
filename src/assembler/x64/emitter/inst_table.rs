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
//!
//! # SSE (浮動小数点 / SIMD) 命令
//!
//! `movss` / `addps` などの SSE 命令は、`dispatch` の `match` ではなく
//! `sse_spec` の表 (ニーモニック -> プレフィックス / オペコード) で管理している。
//! 同じ形式 (`xmm/m` -> `xmm`) の命令を足すときは `sse_spec` に1行足すだけでよい。
//! SSE 以外の命令に `%xmm` / `%ymm` / `%zmm` を渡すとエラーになる。

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

/// cmp系ニーモニックのサフィックスからメモリオペランドのサイズ(bit数)を求める。
/// 即値とメモリの組み合わせはレジスタからサイズを推測できないため必要。
fn cmp_size_bits(mnemonic: &str) -> u32 {
    match mnemonic {
        "cmpb" => 8,
        "cmpw" => 16,
        "cmpq" => 64,
        _ => 32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmp_imm_mem_bytes(mnemonic: &str, value: i64, mem: MemoryOperand) -> Vec<u8> {
        let mut emitter = Emitter::new();
        dispatch(
            &mut emitter,
            &Instruction {
                mnemonic: mnemonic.to_owned(),
                operands: vec![Operand::Immediate(value), Operand::Memory(mem)],
            },
        )
        .unwrap();
        emitter.text
    }

    fn base_mem(base: Reg, displacement: i64) -> MemoryOperand {
        MemoryOperand {
            displacement,
            base: Some(base),
            index: None,
            scale: 1,
            symbol: None,
        }
    }

    #[test]
    fn cmp_immediate_to_memory_uses_mnemonic_width() {
        assert_eq!(
            cmp_imm_mem_bytes("cmpb", -1, base_mem(Reg::Rax, 0)),
            [0x80, 0x38, 0xff]
        );
        assert_eq!(
            cmp_imm_mem_bytes("cmpw", 0x1234, base_mem(Reg::Rax, 0)),
            [0x66, 0x81, 0x38, 0x34, 0x12]
        );
        assert_eq!(
            cmp_imm_mem_bytes("cmpl", 1, base_mem(Reg::Rbp, -4)),
            [0x83, 0x7d, 0xfc, 0x01]
        );
        assert_eq!(
            cmp_imm_mem_bytes("cmpq", 0x12345678, base_mem(Reg::Rax, 0)),
            [0x48, 0x81, 0x38, 0x78, 0x56, 0x34, 0x12]
        );
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

// ============================================================
// SSE 命令の表
// ============================================================

/// SSE 命令の符号化の形
#[derive(Clone, Copy)]
enum SseForm {
    /// `op xmm/m, xmm` (+ `store_opcode` があれば `op xmm, m` も可)
    Op,
    /// `cmpps $imm, xmm/m, xmm`
    CmpPs,
    /// `cmpeqps xmm/m, xmm` など。比較述語が命令名に含まれる (imm8 は固定値)
    CmpPsPredicate(u8),
}

/// SSE 命令1つ分の符号化情報
///
/// 機械語は `[prefix] [REX] 0F opcode ModRM [imm8]` の形になる。
#[derive(Clone, Copy)]
struct SseSpec {
    /// 必須プレフィックス (SS: F3 / SD: F2 / 倍精度の比較: 66 / パック単精度: なし)
    prefix: Option<u8>,
    /// `xmm/m -> xmm` (ロード側) のオペコード
    opcode: u8,
    /// `xmm -> m` (ストア側) のオペコード。ストアできない命令は `None`
    store_opcode: Option<u8>,
    form: SseForm,
}

/// ニーモニックから SSE 命令の符号化情報を引く
fn sse_spec(name: &str) -> Option<SseSpec> {
    const F3: Option<u8> = Some(0xF3);
    const F2: Option<u8> = Some(0xF2);
    const P66: Option<u8> = Some(0x66);

    let op = |prefix, opcode| SseSpec {
        prefix,
        opcode,
        store_opcode: None,
        form: SseForm::Op,
    };
    let mov = |prefix, load, store| SseSpec {
        prefix,
        opcode: load,
        store_opcode: Some(store),
        form: SseForm::Op,
    };
    let cmp = |pred| SseSpec {
        prefix: None,
        opcode: 0xC2,
        store_opcode: None,
        form: SseForm::CmpPsPredicate(pred),
    };

    Some(match name {
        // データ移動
        "movss" => mov(F3, 0x10, 0x11),
        "movsd" => mov(F2, 0x10, 0x11),
        "movups" => mov(None, 0x10, 0x11),
        "movaps" => mov(None, 0x28, 0x29),

        // 加算 / 減算 / 乗算 / 除算 / 平方根
        "addss" => op(F3, 0x58),
        "addsd" => op(F2, 0x58),
        "addps" => op(None, 0x58),
        "subss" => op(F3, 0x5C),
        "subsd" => op(F2, 0x5C),
        "subps" => op(None, 0x5C),
        "mulss" => op(F3, 0x59),
        "mulsd" => op(F2, 0x59),
        "mulps" => op(None, 0x59),
        "divss" => op(F3, 0x5E),
        "divsd" => op(F2, 0x5E),
        "divps" => op(None, 0x5E),
        "sqrtss" => op(F3, 0x51),
        "sqrtsd" => op(F2, 0x51),
        "sqrtps" => op(None, 0x51),

        // 比較 (結果は EFLAGS の ZF/PF/CF)
        "comiss" => op(None, 0x2F),
        "ucomiss" => op(None, 0x2E),
        "comisd" => op(P66, 0x2F),
        "ucomisd" => op(P66, 0x2E),

        // パック比較 (結果は各要素のマスク)
        "cmpps" => SseSpec {
            prefix: None,
            opcode: 0xC2,
            store_opcode: None,
            form: SseForm::CmpPs,
        },
        "cmpeqps" => cmp(0),
        "cmpltps" => cmp(1),
        "cmpleps" => cmp(2),
        "cmpunordps" => cmp(3),
        "cmpneqps" => cmp(4),
        "cmpnltps" => cmp(5),
        "cmpnleps" => cmp(6),
        "cmpordps" => cmp(7),

        _ => return None,
    })
}

/// ニーモニックに対応する処理を呼び出す
pub(super) fn dispatch(e: &mut Emitter, inst: &Instruction) -> Result<(), String> {
    let name = inst.mnemonic.as_str();

    // SSE 命令は表 (`sse_spec`) で引く
    if let Some(spec) = sse_spec(name) {
        return e.emit_sse(name, spec, &inst.operands);
    }

    // SSE 以外の命令はベクトルレジスタを扱えない。
    // (そのまま通すとレジスタ幅を誤認して不正な機械語になるので弾く)
    if inst
        .operands
        .iter()
        .any(|o| matches!(o, Operand::Register(r) if r.is_vector()))
    {
        return Err(format!(
            "{}: vector registers (%xmm/%ymm/%zmm) are not supported by this instruction",
            name
        ));
    }

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

        "cmp" | "cmpq" | "cmpl" | "cmpw" | "cmpb" => {
            e.emit_cmp(&inst.operands, cmp_size_bits(name))
        }

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

/// SSE 命令の ModRM.rm 側のオペランド
enum SseRm<'a> {
    Reg(Reg),
    Mem(&'a MemoryOperand),
}

/// SSE 命令のレジスタオペランドが `%xmm` か確かめる
fn xmm_operand(name: &str, reg: Reg) -> Result<Reg, String> {
    match reg.bits() {
        128 => Ok(reg),
        256 | 512 => Err(format!(
            "{}: %ymm/%zmm registers need VEX/EVEX encoding, which is not supported",
            name
        )),
        _ => Err(format!("{}: operand must be an xmm register", name)),
    }
}

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
    // SSE 命令  ([prefix] [REX] 0F opcode ModRM [imm8])
    //
    //   movss  -4(%rbp), %xmm0     -> F3 0F 10 /r   (ロード)
    //   movss  %xmm0, -4(%rbp)     -> F3 0F 11 /r   (ストア)
    //   addsd  %xmm1, %xmm0        -> F2 0F 58 /r
    //   cmpps  $1, %xmm1, %xmm0    -> 0F C2 /r ib
    //
    // AT&T 構文なので `op 元, 先`。ModRM の reg が `%xmm` 側、rm が
    // もう一方 (レジスタ / メモリ)。ストアだけは reg が元になる。
    //
    // 対応するのは `%xmm0`-`%xmm15` のみ。`%ymm` / `%zmm` は VEX / EVEX
    // 符号化が必要なので、ここではエラーにする。
    // --------------------------------------------------------
    fn emit_sse(&mut self, name: &str, spec: SseSpec, operands: &[Operand]) -> Result<(), String> {
        // cmpps だけ先頭に imm8 が付く
        let (imm, operands) = match spec.form {
            SseForm::Op => (None, operands),
            SseForm::CmpPsPredicate(pred) => (Some(pred), operands),
            SseForm::CmpPs => match operands.split_first() {
                Some((Operand::Immediate(value), rest)) => {
                    if !(0..=7).contains(value) {
                        return Err(format!(
                            "{}: comparison predicate must be in 0..=7 (got {})",
                            name, value
                        ));
                    }
                    (Some(*value as u8), rest)
                }
                _ => return Err(format!("{}: first operand must be an immediate", name)),
            },
        };

        if operands.len() != 2 {
            return Err(format!(
                "{} requires {} operands",
                name,
                if matches!(spec.form, SseForm::CmpPs) { 3 } else { 2 }
            ));
        }

        // `foo(%rip)` のようなシンボル参照は、命令の末尾の imm8 の分だけ
        // RIP からの距離がずれる。ここの再配置は未対応なので弾く。
        if imm.is_some() {
            if let Operand::Memory(mem) = &operands[0] {
                if mem.base == Some(Reg::Rip) && mem.symbol.is_some() {
                    return Err(format!(
                        "{}: symbol(%rip) operand with an immediate is not supported",
                        name
                    ));
                }
            }
        }

        // (opcode, ModRM.reg に入れる xmm, rm 側のオペランド)
        let (opcode, reg, rm) = match (&operands[0], &operands[1]) {
            (Operand::Register(src), Operand::Register(dst)) => {
                (spec.opcode, xmm_operand(name, *dst)?, SseRm::Reg(xmm_operand(name, *src)?))
            }
            (Operand::Memory(mem), Operand::Register(dst)) => {
                (spec.opcode, xmm_operand(name, *dst)?, SseRm::Mem(mem))
            }
            (Operand::Register(src), Operand::Memory(mem)) => match spec.store_opcode {
                Some(store) => (store, xmm_operand(name, *src)?, SseRm::Mem(mem)),
                None => {
                    return Err(format!("{}: destination must be an xmm register", name));
                }
            },
            _ => return Err(format!("unsupported {} operands", name)),
        };

        if let Some(prefix) = spec.prefix {
            self.emit_u8(prefix);
        }

        match rm {
            SseRm::Reg(rm) => {
                self.emit_rex(false, reg.id() >= 8, false, rm.id() >= 8);
                self.emit_u8(0x0F);
                self.emit_u8(opcode);
                self.emit_u8(0b11_000_000 | ((reg.id() & 7) << 3) | (rm.id() & 7));
            }
            SseRm::Mem(mem) => {
                self.emit_rex(
                    false,
                    reg.id() >= 8,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                );
                self.emit_u8(0x0F);
                self.emit_u8(opcode);
                self.emit_memory_modrm(reg.id(), mem)?;
            }
        }

        if let Some(imm) = imm {
            self.emit_u8(imm);
        }

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
#[cfg(test)]
mod sse_tests {
    use crate::assembler::x64::{convert, Lexer, Parser};

    use super::*;

    fn assemble(source: &str) -> Result<Vec<u8>, String> {
        let tokens = Lexer::new(source).tokenize();
        let ast = Parser::new(tokens).parse()?;
        let program = convert::convert_program(&ast)?;
        let mut emitter = Emitter::new();
        emitter.emit_program(&program)?;
        Ok(emitter.text)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// GNU as (binutils) の出力と一致するか。期待値は `as` + `objdump` で生成したもの
    #[test]
    fn sse_matches_gnu_as() {
        let cases: &[(&str, &str)] = &[
        ("movss %xmm1, %xmm0", "f30f10c1"),
        ("movss -4(%rbp), %xmm0", "f30f1045fc"),
        ("movss %xmm0, -4(%rbp)", "f30f1145fc"),
        ("movss %xmm9, %xmm2", "f3410f10d1"),
        ("movss %xmm2, %xmm9", "f3440f10ca"),
        ("movss 8(%r12,%r13,4), %xmm10", "f3470f1054ac08"),
        ("movss %xmm15, (%rsp)", "f3440f113c24"),
        ("movsd %xmm1, %xmm0", "f20f10c1"),
        ("movsd 16(%rax), %xmm3", "f20f105810"),
        ("movsd %xmm3, 16(%rax)", "f20f115810"),
        ("movups %xmm1, %xmm0", "0f10c1"),
        ("movups (%rdi), %xmm8", "440f1007"),
        ("movups %xmm8, 32(%rdi)", "440f114720"),
        ("movaps %xmm1, %xmm0", "0f28c1"),
        ("movaps (%rsi), %xmm1", "0f280e"),
        ("movaps %xmm1, 256(%rsi)", "0f298e00010000"),
        ("addss %xmm1, %xmm0", "f30f58c1"),
        ("addsd %xmm1, %xmm0", "f20f58c1"),
        ("addps %xmm1, %xmm0", "0f58c1"),
        ("subss -8(%rbp), %xmm0", "f30f5c45f8"),
        ("subsd %xmm9, %xmm1", "f2410f5cc9"),
        ("subps %xmm1, %xmm12", "440f5ce1"),
        ("mulss %xmm1, %xmm0", "f30f59c1"),
        ("mulsd (%rax,%rcx,8), %xmm0", "f20f5904c8"),
        ("mulps %xmm1, %xmm0", "0f59c1"),
        ("divss %xmm1, %xmm0", "f30f5ec1"),
        ("divsd %xmm10, %xmm11", "f2450f5eda"),
        ("divps (%rax), %xmm0", "0f5e00"),
        ("sqrtss %xmm1, %xmm0", "f30f51c1"),
        ("sqrtsd -16(%rbp), %xmm0", "f20f5145f0"),
        ("sqrtps %xmm14, %xmm15", "450f51fe"),
        ("comiss %xmm1, %xmm0", "0f2fc1"),
        ("ucomiss -4(%rbp), %xmm0", "0f2e45fc"),
        ("comisd %xmm1, %xmm0", "660f2fc1"),
        ("ucomisd %xmm9, %xmm8", "66450f2ec1"),
        ("cmpps $0, %xmm1, %xmm0", "0fc2c100"),
        ("cmpps $7, (%rax), %xmm9", "440fc20807"),
        ("cmpps $1, -32(%rbp), %xmm0", "0fc245e001"),
        ("cmpeqps %xmm1, %xmm0", "0fc2c100"),
        ("cmpltps %xmm1, %xmm0", "0fc2c101"),
        ("cmpleps %xmm1, %xmm0", "0fc2c102"),
        ("cmpunordps %xmm1, %xmm0", "0fc2c103"),
        ("cmpneqps %xmm1, %xmm0", "0fc2c104"),
        ("cmpnltps %xmm1, %xmm0", "0fc2c105"),
        ("cmpnleps %xmm1, %xmm0", "0fc2c106"),
        ("cmpordps %xmm1, %xmm0", "0fc2c107"),
        ("movss 4(%rip), %xmm0", "f30f100504000000"),
        ];

        for (asm, expected) in cases {
            let bytes = assemble(asm).unwrap_or_else(|e| panic!("{}: {}", asm, e));
            assert_eq!(hex(&bytes), *expected, "{}", asm);
        }
    }

    #[test]
    fn sse_rejects_invalid_operands() {
        let cases = [
            "addss %ymm1, %ymm0",        // VEX が必要
            "addps %zmm1, %zmm0",        // EVEX が必要
            "addss %eax, %xmm0",         // 汎用レジスタ
            "addss %xmm0, -4(%rbp)",     // addss にストア形式はない
            "cmpps $8, %xmm1, %xmm0",    // 述語は 0..=7
            "cmpps %xmm1, %xmm0",        // imm8 がない
            "cmpps $1, foo(%rip), %xmm0", // imm8 付き RIP 相対シンボルは未対応
            "movss %xmm0",               // オペランド不足
            "add %xmm0, %xmm1",          // SSE 以外の命令はベクトルレジスタ不可
            "mov %xmm0, %rax",
        ];

        for asm in cases {
            assert!(assemble(asm).is_err(), "{} should fail", asm);
        }
    }
}
