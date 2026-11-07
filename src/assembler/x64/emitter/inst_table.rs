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
//!    条件付きジャンプ (`jl` / `jne` など) と条件付き移動 (`cmovl` / `cmovne` など) は
//!    下の `JCC_TABLE` (ニーモニック -> 条件) で管理する。
//!    条件付きバイトセット (`sete` / `setl` / `setb` など) も同じ表を使う。
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

    #[test]
    fn movzbl_zero_extends_register_and_memory_sources() {
        assert_eq!(
            reg_reg_bytes("movzbl", Reg::Al, Reg::Ecx),
            [0x0F, 0xB6, 0xC8]
        );
        assert_eq!(
            reg_reg_bytes("movzbl", Reg::Sil, Reg::R8d),
            [0x44, 0x0F, 0xB6, 0xC6]
        );
        assert_eq!(
            reg_reg_bytes("movzbl", Reg::R9b, Reg::R10d),
            [0x45, 0x0F, 0xB6, 0xD1]
        );

        let mut emitter = Emitter::new();
        dispatch(
            &mut emitter,
            &Instruction {
                mnemonic: "movzbl".to_owned(),
                operands: vec![
                    Operand::Memory(base_mem(Reg::Rbp, -4)),
                    Operand::Register(Reg::R8d),
                ],
            },
        )
        .unwrap();
        assert_eq!(emitter.text, [0x44, 0x0F, 0xB6, 0x45, 0xFC]);
    }

    #[test]
    fn movzbl_rejects_invalid_operand_widths() {
        for operands in [
            vec![Operand::Register(Reg::Ax), Operand::Register(Reg::Eax)],
            vec![Operand::Register(Reg::Al), Operand::Register(Reg::Rax)],
            vec![Operand::Immediate(1), Operand::Register(Reg::Eax)],
        ] {
            let mut emitter = Emitter::new();
            assert!(
                dispatch(
                    &mut emitter,
                    &Instruction {
                        mnemonic: "movzbl".to_owned(),
                        operands,
                    },
                )
                .is_err()
            );
        }
    }

    #[test]
    fn movzbq_zero_extends_register_and_memory_sources() {
        assert_eq!(
            reg_reg_bytes("movzbq", Reg::Al, Reg::Rax),
            [0x48, 0x0F, 0xB6, 0xC0]
        );
        assert_eq!(
            reg_reg_bytes("movzbq", Reg::Sil, Reg::R8),
            [0x4C, 0x0F, 0xB6, 0xC6]
        );
        assert_eq!(
            reg_reg_bytes("movzbq", Reg::R9b, Reg::R10),
            [0x4D, 0x0F, 0xB6, 0xD1]
        );

        let mut emitter = Emitter::new();
        dispatch(
            &mut emitter,
            &Instruction {
                mnemonic: "movzbq".to_owned(),
                operands: vec![
                    Operand::Memory(base_mem(Reg::Rbp, -4)),
                    Operand::Register(Reg::R8),
                ],
            },
        )
        .unwrap();
        assert_eq!(emitter.text, [0x4C, 0x0F, 0xB6, 0x45, 0xFC]);
    }

    #[test]
    fn movzbq_rejects_invalid_operand_widths() {
        for operands in [
            vec![Operand::Register(Reg::Ax), Operand::Register(Reg::Rax)],
            vec![Operand::Register(Reg::Al), Operand::Register(Reg::Eax)],
            vec![Operand::Immediate(1), Operand::Register(Reg::Rax)],
        ] {
            let mut emitter = Emitter::new();
            assert!(
                dispatch(
                    &mut emitter,
                    &Instruction {
                        mnemonic: "movzbq".to_owned(),
                        operands,
                    },
                )
                .is_err()
            );
        }
    }

    fn reg_reg_bytes(mnemonic: &str, src: Reg, dst: Reg) -> Vec<u8> {
        let mut emitter = Emitter::new();
        dispatch(
            &mut emitter,
            &Instruction {
                mnemonic: mnemonic.to_owned(),
                operands: vec![Operand::Register(src), Operand::Register(dst)],
            },
        )
        .unwrap();
        emitter.text
    }

    #[test]
    fn lea_register_to_register_is_mov() {
        // leaq %rax, %rcx == movq %rax, %rcx
        assert_eq!(
            reg_reg_bytes("leaq", Reg::Rax, Reg::Rcx),
            [0x48, 0x89, 0xc1]
        );
        assert_eq!(
            reg_reg_bytes("leaq", Reg::Rax, Reg::Rcx),
            reg_reg_bytes("movq", Reg::Rax, Reg::Rcx)
        );
        // leal %eax, %ecx
        assert_eq!(reg_reg_bytes("leal", Reg::Eax, Reg::Ecx), [0x89, 0xc1]);
    }
}

/// neg / not 系ニーモニックのサフィックスからオペランドサイズ(bit数)を求める。
/// サフィックス無しの `neg` / `not` は `None` を返す。
/// レジスタ操作ならレジスタ名からサイズが決まるが、`negl -4(%rbp)` のような
/// メモリ操作ではサフィックスだけが手がかりになる。
fn neg_suffix_bits(mnemonic: &str) -> Option<u32> {
    match mnemonic {
        "negb" | "notb" => Some(8),
        "negw" | "notw" => Some(16),
        "negl" | "notl" => Some(32),
        "negq" | "notq" => Some(64),
        _ => None,
    }
}

/// `base` (`and` / `shl` など) にサイズ接尾辞 (`b`/`w`/`l`/`q`) が付いた
/// ニーモニックから、オペランドサイズ(bit数)を求める。接尾辞無しは `None`。
/// レジスタ操作ならレジスタ名からサイズが決まるが、`andl $1, -4(%rbp)` のような
/// メモリ操作ではサフィックスだけが手がかりになる。
fn mnemonic_suffix_bits(mnemonic: &str, base: &str) -> Option<u32> {
    match mnemonic.strip_prefix(base)? {
        "b" => Some(8),
        "w" => Some(16),
        "l" => Some(32),
        "q" => Some(64),
        _ => None,
    }
}

// ============================================================
// 文字列命令 / rep プレフィックスの表
// ============================================================

/// 文字列命令の符号化 (オペコードとオペランドサイズ)
#[derive(Clone, Copy)]
struct StringOp {
    opcode: u8,
    bits: u32,
}

/// `rep` 系プレフィックスのバイトを返す。
///
/// `rep` / `repe` / `repz` は `F3`、`repne` / `repnz` は `F2`。
/// GNU as と同じく、どのプレフィックスも5種類の文字列命令すべてに付けられる
/// (`rep` を `cmps` / `scas` に付けると `repe` と同じ `F3` になる)。
fn rep_prefix(name: &str) -> Option<u8> {
    match name {
        "rep" | "repe" | "repz" => Some(0xF3),
        "repne" | "repnz" => Some(0xF2),
        _ => None,
    }
}

/// 文字列命令 (`movsb` / `cmpsw` / `stosl` / `lodsq` / `scasb` など) の符号化を返す。
///
/// 末尾のサフィックスが `b` / `w` / `l` / `q` のとき 8 / 16 / 32 / 64 bit。
/// 8bit 版のオペコードに 1 を足したものが 16/32/64bit 版のオペコード。
/// `movsd` / `cmpsd` は SSE 命令と同じ名前なので、オペランドが無いとき
/// (`no_operands`) だけ 32bit の文字列命令として扱う (GNU as は `movsl` / `cmpsl` とみなす)。
fn string_op_spec(name: &str, no_operands: bool) -> Option<StringOp> {
    const FAMILIES: [(&str, u8); 5] = [
        ("movs", 0xA4),
        ("cmps", 0xA6),
        ("stos", 0xAA),
        ("lods", 0xAC),
        ("scas", 0xAE),
    ];

    for (stem, byte_opcode) in FAMILIES {
        let Some(suffix) = name.strip_prefix(stem) else {
            continue;
        };

        let bits = match suffix {
            "b" => 8,
            "w" => 16,
            "l" => 32,
            "q" => 64,
            "d" if no_operands && (stem == "movs" || stem == "cmps") => 32,
            _ => return None,
        };

        let opcode = if bits == 8 {
            byte_opcode
        } else {
            byte_opcode + 1
        };

        return Some(StringOp { opcode, bits });
    }

    None
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
    /// 整数レジスタ/メモリから浮動小数点レジスタへ変換
    IntToFloat,
    /// 浮動小数点レジスタ/メモリから整数レジスタへ切り捨て変換
    FloatToInt,
    /// 単精度と倍精度の変換
    FloatPrecision,
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

        // 整数と浮動小数点の変換
        "cvtsi2ss" | "cvtsi2ssl" | "cvtsi2ssq" => SseSpec {
            prefix: F3,
            opcode: 0x2A,
            store_opcode: None,
            form: SseForm::IntToFloat,
        },
        "cvtsi2sd" | "cvtsi2sdl" | "cvtsi2sdq" => SseSpec {
            prefix: F2,
            opcode: 0x2A,
            store_opcode: None,
            form: SseForm::IntToFloat,
        },
        "cvttss2si" | "cvttss2sil" | "cvttss2siq" => SseSpec {
            prefix: F3,
            opcode: 0x2C,
            store_opcode: None,
            form: SseForm::FloatToInt,
        },
        "cvttsd2si" | "cvttsd2sil" | "cvttsd2siq" => SseSpec {
            prefix: F2,
            opcode: 0x2C,
            store_opcode: None,
            form: SseForm::FloatToInt,
        },
        "cvtss2sd" => SseSpec {
            prefix: F3,
            opcode: 0x5A,
            store_opcode: None,
            form: SseForm::FloatPrecision,
        },
        "cvtsd2ss" => SseSpec {
            prefix: F2,
            opcode: 0x5A,
            store_opcode: None,
            form: SseForm::FloatPrecision,
        },

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

/// 条件付きジャンプ (Jcc) のニーモニックと条件の対応表 (別名は同じ行に並べる)
///
/// x86-64 の 16 種類の条件すべてに、GNU as が受け付ける別名を含めている。
const JCC_TABLE: &[(&[&str], Condition)] = &[
    (&["jo"], Condition::O),
    (&["jno"], Condition::NO),
    (&["jb", "jc", "jnae"], Condition::B),
    (&["jae", "jnb", "jnc"], Condition::AE),
    (&["je", "jz"], Condition::E),
    (&["jne", "jnz"], Condition::NE),
    (&["jbe", "jna"], Condition::BE),
    (&["ja", "jnbe"], Condition::A),
    (&["js"], Condition::S),
    (&["jns"], Condition::NS),
    (&["jp", "jpe"], Condition::P),
    (&["jnp", "jpo"], Condition::NP),
    (&["jl", "jnge"], Condition::L),
    (&["jge", "jnl"], Condition::GE),
    (&["jle", "jng"], Condition::LE),
    (&["jg", "jnle"], Condition::G),
];

/// 条件付きジャンプのニーモニックから条件を求める (Jcc でなければ `None`)
fn jcc_condition(name: &str) -> Option<Condition> {
    JCC_TABLE
        .iter()
        .find(|(names, _)| names.contains(&name))
        .map(|&(_, condition)| condition)
}

/// 条件付き移動のニーモニックから条件とサイズ接尾辞を求める。
fn cmov_spec(name: &str) -> Option<(Condition, Option<u32>)> {
    let name = name.strip_prefix("cmov")?;
    let condition = |name: &str| {
        JCC_TABLE
            .iter()
            .find(|(names, _)| names.iter().any(|jcc| jcc.strip_prefix('j') == Some(name)))
            .map(|&(_, condition)| condition)
    };

    if let Some(condition) = condition(name) {
        return Some((condition, None));
    }

    let (condition_name, bits) = match name.chars().last()? {
        'w' => (name.strip_suffix('w')?, 16),
        'l' => (name.strip_suffix('l')?, 32),
        'q' => (name.strip_suffix('q')?, 64),
        _ => return None,
    };
    if condition_name.is_empty() {
        return None;
    }

    condition(condition_name).map(|condition| (condition, Some(bits)))
}

/// 条件付きバイトセット (`setcc`) のニーモニックから条件を求める (`setcc` でなければ `None`)
///
/// `JCC_TABLE` の `j` を `set` に置き換えたニーモニック (`setl` / `setnge` / `setz` ...) が対象。
/// `setcc` にサイズ接尾辞は無い (オペランドは常に8bit)。
fn setcc_condition(name: &str) -> Option<Condition> {
    let name = name.strip_prefix("set")?;
    JCC_TABLE
        .iter()
        .find(|(names, _)| names.iter().any(|jcc| jcc.strip_prefix('j') == Some(name)))
        .map(|&(_, condition)| condition)
}

/// ニーモニックに対応する処理を呼び出す
pub(super) fn dispatch(e: &mut Emitter, inst: &Instruction) -> Result<(), String> {
    let name = inst.mnemonic.as_str();

    // `rep` / `repe` / `repz` / `repne` / `repnz` + 文字列命令 (`rep movsb` など)
    if let Some(prefix) = rep_prefix(name) {
        return e.emit_rep(name, prefix, &inst.operands);
    }

    // 文字列命令 (`movsb` / `stosq` / `scasb` ...)。オペランドは暗黙 (%rsi / %rdi / %al ...)。
    // `movsd` / `cmpsd` はオペランドが無いときだけここで扱い、有るときは SSE 命令になる。
    if let Some(op) = string_op_spec(name, inst.operands.is_empty()) {
        return e.emit_string_op(name, op, None, &inst.operands);
    }

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

    if let Some((condition, suffix_bits)) = cmov_spec(name) {
        return e.emit_cmov(condition, suffix_bits, &inst.operands);
    }

    if let Some(condition) = setcc_condition(name) {
        return e.emit_setcc(name, condition, &inst.operands);
    }

    match name {
        "push" | "pushq" | "pushl" | "pushw" => e.emit_push(&inst.operands),

        "pop" | "popq" | "popl" | "popw" => e.emit_pop(&inst.operands),

        "inc" | "incq" | "incl" | "incw" | "incb" => {
            e.emit_inc_dec(&inst.operands, 0, mnemonic_suffix_bits(name, "inc"))
        }
        "dec" | "decq" | "decl" | "decw" | "decb" => {
            e.emit_inc_dec(&inst.operands, 1, mnemonic_suffix_bits(name, "dec"))
        }

        "mov" | "movq" | "movl" | "movw" | "movb" => {
            e.emit_mov(&inst.operands, mov_size_bits(name))
        }

        "add" | "addq" | "addl" | "addw" | "addb" => e.emit_add(&inst.operands),

        "sub" | "subq" | "subl" | "subw" | "subb" => e.emit_sub(&inst.operands),

        "imul" | "imulq" | "imull" | "imulw" => {
            e.emit_imul(&inst.operands, mnemonic_suffix_bits(name, "imul"))
        }

        "mul" | "mulq" | "mull" | "mulw" => e.emit_mul(&inst.operands),

        "div" | "divq" | "divl" | "divw" => e.emit_div(&inst.operands),

        "idiv" | "idivq" | "idivl" | "idivw" => e.emit_idiv(&inst.operands),

        "cltd" | "cdq" => e.emit_cltd(),

        "cqto" | "cqo" => e.emit_cqto(&inst.operands),

        "neg" | "negq" | "negl" | "negw" | "negb" => {
            e.emit_neg(&inst.operands, neg_suffix_bits(name))
        }

        "not" | "notq" | "notl" | "notw" | "notb" => {
            e.emit_not(&inst.operands, neg_suffix_bits(name))
        }

        "movslq" => e.emit_movslq(&inst.operands),

        "movzbl" => e.emit_movzb(&inst.operands, 32),

        "movzbq" => e.emit_movzb(&inst.operands, 64),

        "cmp" | "cmpq" | "cmpl" | "cmpw" | "cmpb" => {
            e.emit_cmp(&inst.operands, cmp_size_bits(name))
        }

        "and" | "andq" | "andl" | "andw" | "andb" => e.emit_alu(
            AluOp::And,
            &inst.operands,
            mnemonic_suffix_bits(name, "and"),
        ),

        "or" | "orq" | "orl" | "orw" | "orb" => {
            e.emit_alu(AluOp::Or, &inst.operands, mnemonic_suffix_bits(name, "or"))
        }

        "xor" | "xorq" | "xorl" | "xorw" | "xorb" => e.emit_alu(
            AluOp::Xor,
            &inst.operands,
            mnemonic_suffix_bits(name, "xor"),
        ),

        // `sal` は `shl` と同じ命令 (左シフトは算術/論理で同じ)
        "shl" | "shlq" | "shll" | "shlw" | "shlb" => e.emit_shift(
            ShiftOp::Shl,
            &inst.operands,
            mnemonic_suffix_bits(name, "shl"),
        ),

        "sal" | "salq" | "sall" | "salw" | "salb" => e.emit_shift(
            ShiftOp::Shl,
            &inst.operands,
            mnemonic_suffix_bits(name, "sal"),
        ),

        "shr" | "shrq" | "shrl" | "shrw" | "shrb" => e.emit_shift(
            ShiftOp::Shr,
            &inst.operands,
            mnemonic_suffix_bits(name, "shr"),
        ),

        "sar" | "sarq" | "sarl" | "sarw" | "sarb" => e.emit_shift(
            ShiftOp::Sar,
            &inst.operands,
            mnemonic_suffix_bits(name, "sar"),
        ),

        "lea" | "leaq" | "leal" => e.emit_lea(&inst.operands),

        // 条件付きジャンプ (`jo` ... `jg`、別名を含む) は `JCC_TABLE` で引く
        _ if jcc_condition(name).is_some() => match jcc_condition(name) {
            Some(condition) => e.emit_jcc(condition, &inst.operands),
            None => Err(format!("unsupported instruction: {}", name)),
        },

        // CX/ECX/RCX が 0 ならジャンプ (8bit 相対のみ。64bitモードに `jcxz` は無い)
        "jrcxz" => e.emit_jcxz(name, false, &inst.operands),

        "jecxz" => e.emit_jcxz(name, true, &inst.operands),

        "jmp" => e.emit_jmp(&inst.operands),

        "call" => e.emit_call(&inst.operands),

        "ret" => e.emit_ret(&inst.operands),

        "leave" => e.emit_leave(&inst.operands),

        "syscall" => e.emit_syscall(&inst.operands),

        "nop" => e.emit_nop(&inst.operands),

        // DF (方向フラグ) のクリア / セット。文字列命令の進む向きを決める
        "cld" => e.emit_simple(name, 0xFC, &inst.operands),

        "std" => e.emit_simple(name, 0xFD, &inst.operands),

        "test" | "testq" | "testl" | "testw" | "testb" => {
            e.emit_test(&inst.operands, mnemonic_suffix_bits(name, "test"))
        }

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
    fn emit_cmov(
        &mut self,
        condition: Condition,
        suffix_bits: Option<u32>,
        operands: &[Operand],
    ) -> Result<(), String> {
        let [src, Operand::Register(dst)] = operands else {
            return Err(
                "conditional move requires a register destination and register or memory source"
                    .into(),
            );
        };

        let bits = dst.bits() as u32;
        if !matches!(bits, 16 | 32 | 64) {
            return Err("conditional move requires a 16-, 32-, or 64-bit destination".into());
        }
        Self::check_suffix("cmov", suffix_bits, bits)?;

        let (src_reg, mem) = match src {
            Operand::Register(reg) => {
                if reg.bits() as u32 != bits {
                    return Err("conditional move source and destination sizes do not match".into());
                }
                (Some(*reg), None)
            }
            Operand::Memory(mem) => (None, Some(mem)),
            _ => {
                return Err("conditional move source must be a register or memory operand".into());
            }
        };

        let (x, b) = match (src_reg, mem) {
            (Some(src), None) => (false, src.id() >= 8),
            (None, Some(mem)) => (
                self.memory_index_extended(mem),
                self.memory_base_extended(mem),
            ),
            _ => unreachable!(),
        };
        self.emit_sized_prefix(bits, dst.id() >= 8, x, b, false);
        self.emit_u8(0x0F);
        self.emit_u8(0x40 | condition as u8);

        match (src_reg, mem) {
            (Some(src), None) => {
                self.emit_u8(0b11_000_000 | ((dst.id() & 7) << 3) | (src.id() & 7));
                Ok(())
            }
            (None, Some(mem)) => self.emit_memory_modrm(dst.id(), mem),
            _ => unreachable!(),
        }
    }

    // --------------------------------------------------------
    // setcc  (条件が真なら1、偽なら0を8bitのレジスタ/メモリへ書く)
    //
    //   sete  %al          -> 0F 94 /0
    //   setl  %cl          -> 0F 9C C1
    //   setb  %sil         -> 40 0F 92 C6     (%spl/%bpl/%sil/%dil は空の REX が必要)
    //   setne %r8b         -> 41 0F 95 C0
    //   setle 8(%r12)      -> 41 0F 9E 44 24 08
    //
    // オペコードは `0F 90+cc`、ModRM の reg は使わない (0)。
    // --------------------------------------------------------
    fn emit_setcc(
        &mut self,
        name: &str,
        condition: Condition,
        operands: &[Operand],
    ) -> Result<(), String> {
        let [operand] = operands else {
            return Err(format!("{} requires one operand", name));
        };

        match operand {
            Operand::Register(reg) => {
                if reg.bits() != 8 {
                    return Err(format!("{}: operand must be an 8-bit register", name));
                }
                self.emit_sized_prefix(8, false, false, reg.id() >= 8, needs_empty_rex(*reg));
                self.emit_u8(0x0F);
                self.emit_u8(0x90 | condition as u8);
                self.emit_u8(0b11_000_000 | (reg.id() & 7));
                Ok(())
            }
            Operand::Memory(mem) => {
                self.emit_sized_prefix(
                    8,
                    false,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                    false,
                );
                self.emit_u8(0x0F);
                self.emit_u8(0x90 | condition as u8);
                self.emit_memory_modrm(0, mem)
            }
            operand => Err(format!(
                "{}: expected an 8-bit register or memory operand, got {:?}",
                name, operand
            )),
        }
    }

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
    // オペランドもサイズも無い1バイト命令 (`cld` / `std`)
    // --------------------------------------------------------
    fn emit_simple(&mut self, name: &str, opcode: u8, operands: &[Operand]) -> Result<(), String> {
        if !operands.is_empty() {
            return Err(format!("{} takes no operands", name));
        }
        self.emit_u8(opcode);

        Ok(())
    }

    // --------------------------------------------------------
    // 文字列命令  (オペランドは暗黙)
    //
    //   movsb                    -> A4              [%rdi] <- [%rsi]
    //   movsw / movsl / movsq    -> 66 A5 / A5 / 48 A5
    //   cmpsb ...                -> A6 / A7         [%rsi] と [%rdi] を比較
    //   stosb ...                -> AA / AB         [%rdi] <- %al/%ax/%eax/%rax
    //   lodsb ...                -> AC / AD         %al/%ax/%eax/%rax <- [%rsi]
    //   scasb ...                -> AE / AF         %al/%ax/%eax/%rax と [%rdi] を比較
    //
    // プレフィックスの並びは GNU as と同じ `[66] [F3/F2] [REX.W] opcode`。
    // --------------------------------------------------------
    fn emit_string_op(
        &mut self,
        name: &str,
        op: StringOp,
        prefix: Option<u8>,
        operands: &[Operand],
    ) -> Result<(), String> {
        if !operands.is_empty() {
            return Err(format!(
                "{}: string instructions take no operands (they use %rsi / %rdi / %al implicitly)",
                name
            ));
        }

        if op.bits == 16 {
            self.emit_u8(0x66);
        }
        if let Some(prefix) = prefix {
            self.emit_u8(prefix);
        }
        if op.bits == 64 {
            self.emit_u8(0x48);
        }
        self.emit_u8(op.opcode);

        Ok(())
    }

    // --------------------------------------------------------
    // rep / repe / repz / repne / repnz + 文字列命令
    //
    //   rep movsb      -> F3 A4
    //   repne scasb    -> F2 AE
    //
    // パーサは `rep movsb` を「ニーモニック `rep` + シンボル `movsb`」と読むので、
    // ここでシンボルを文字列命令の名前として取り出し、プレフィックス付きで出力する。
    // --------------------------------------------------------
    fn emit_rep(&mut self, name: &str, prefix: u8, operands: &[Operand]) -> Result<(), String> {
        let [Operand::Symbol(inner)] = operands else {
            return Err(format!(
                "{}: must be followed by a string instruction on the same line (movs / cmps / stos / lods / scas)",
                name
            ));
        };

        let inner = inner.to_ascii_lowercase();

        let Some(op) = string_op_spec(&inner, true) else {
            return Err(format!(
                "{}: only string instructions can follow this prefix, got {}",
                name, inner
            ));
        };

        self.emit_string_op(&inner, op, Some(prefix), &[])
    }

    // --------------------------------------------------------
    // test  (AND の結果でフラグだけ更新する。結果は捨てる)
    //
    //   testb %dl, %dl         -> 84 d2
    //   testl %eax, %ecx       -> 85 c1
    //   testb $1, %al          -> A8 ib            (アキュムレータの短縮形)
    //   testl $1, %ecx         -> F7 /0 id         (imm8 の形は無いので常に全幅)
    //   testl $1, -4(%rbp)     -> F7 /0 id
    //   testl %eax, -4(%rbp)   -> 85 /r
    //
    // `test` は左右を入れ替えても結果が同じなので、`mem, reg` の形も `reg, mem` と同じ符号化。
    // --------------------------------------------------------
    fn emit_test(&mut self, operands: &[Operand], suffix_bits: Option<u32>) -> Result<(), String> {
        if operands.len() != 2 {
            return Err("test requires two operands".into());
        }

        match (&operands[0], &operands[1]) {
            (Operand::Immediate(value), Operand::Register(dst)) => {
                let bits = dst.bits() as u32;
                if !matches!(bits, 8 | 16 | 32 | 64) {
                    return Err("test requires an 8-, 16-, 32-, or 64-bit register".into());
                }
                Self::check_suffix("test", suffix_bits, bits)?;
                let value = normalize_imm("test", *value, bits)?;

                if dst.id() == 0 {
                    self.emit_sized_prefix(bits, false, false, false, false);
                    self.emit_u8(if bits == 8 { 0xA8 } else { 0xA9 });
                } else {
                    self.emit_sized_prefix(
                        bits,
                        false,
                        false,
                        dst.id() >= 8,
                        needs_empty_rex(*dst),
                    );
                    self.emit_u8(if bits == 8 { 0xF6 } else { 0xF7 });
                    self.emit_u8(0b11_000_000 | (dst.id() & 7));
                }
                self.emit_test_imm(value, bits);
                Ok(())
            }

            (Operand::Immediate(value), Operand::Memory(mem)) => {
                let bits = suffix_bits.unwrap_or(32);
                reject_rip_symbol_with_imm("test", mem)?;
                let value = normalize_imm("test", *value, bits)?;

                self.emit_sized_prefix(
                    bits,
                    false,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                    false,
                );
                self.emit_u8(if bits == 8 { 0xF6 } else { 0xF7 });
                self.emit_memory_modrm(0, mem)?;
                self.emit_test_imm(value, bits);
                Ok(())
            }

            (Operand::Register(src), Operand::Register(dst)) => {
                if src.bits() != dst.bits() {
                    return Err("test: register size mismatch".into());
                }
                let bits = src.bits() as u32;
                if !matches!(bits, 8 | 16 | 32 | 64) {
                    return Err("test requires 8-, 16-, 32-, or 64-bit registers".into());
                }
                Self::check_suffix("test", suffix_bits, bits)?;

                self.emit_sized_prefix(
                    bits,
                    src.id() >= 8,
                    false,
                    dst.id() >= 8,
                    needs_empty_rex(*src) || needs_empty_rex(*dst),
                );
                self.emit_u8(if bits == 8 { 0x84 } else { 0x85 });
                self.emit_u8(0b11_000_000 | ((src.id() & 7) << 3) | (dst.id() & 7));
                Ok(())
            }

            (Operand::Register(reg), Operand::Memory(mem))
            | (Operand::Memory(mem), Operand::Register(reg)) => {
                let bits = reg.bits() as u32;
                if !matches!(bits, 8 | 16 | 32 | 64) {
                    return Err("test requires an 8-, 16-, 32-, or 64-bit register".into());
                }
                Self::check_suffix("test", suffix_bits, bits)?;

                self.emit_sized_prefix(
                    bits,
                    reg.id() >= 8,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                    needs_empty_rex(*reg),
                );
                self.emit_u8(if bits == 8 { 0x84 } else { 0x85 });
                self.emit_memory_modrm(reg.id(), mem)
            }

            (l, r) => Err(format!("unsupported test operands {:?} {:?}", l, r)),
        }
    }

    /// `test` の即値部分 (オペランドサイズの幅で出力する。64bit は 32bit を符号拡張)
    fn emit_test_imm(&mut self, value: i64, bits: u32) {
        match bits {
            8 => self.emit_u8(value as u8),
            16 => self.emit_u16(value as u16),
            _ => self.emit_i32(value as i32),
        }
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
        match spec.form {
            SseForm::IntToFloat | SseForm::FloatToInt | SseForm::FloatPrecision => {
                return self.emit_sse_conversion(name, spec, operands);
            }
            _ => {}
        }

        // cmpps だけ先頭に imm8 が付く
        let (imm, operands) = match spec.form {
            SseForm::Op => (None, operands),
            SseForm::CmpPsPredicate(pred) => (Some(pred), operands),
            SseForm::IntToFloat | SseForm::FloatToInt | SseForm::FloatPrecision => unreachable!(),
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
                if matches!(spec.form, SseForm::CmpPs) {
                    3
                } else {
                    2
                }
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
            (Operand::Register(src), Operand::Register(dst)) => (
                spec.opcode,
                xmm_operand(name, *dst)?,
                SseRm::Reg(xmm_operand(name, *src)?),
            ),
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

    fn emit_sse_conversion(
        &mut self,
        name: &str,
        spec: SseSpec,
        operands: &[Operand],
    ) -> Result<(), String> {
        if operands.len() != 2 {
            return Err(format!("{} requires two operands", name));
        }

        let suffix_bits = if name.ends_with('l') {
            Some(32)
        } else if name.ends_with('q') {
            Some(64)
        } else {
            None
        };

        let (reg, rm, w) = match (spec.form, &operands[0], &operands[1]) {
            (SseForm::IntToFloat, Operand::Register(src), Operand::Register(dst)) => {
                let bits = src.bits() as u32;
                if *src == Reg::Rip || !matches!(bits, 32 | 64) {
                    return Err(format!(
                        "{}: integer source must be a 32-bit or 64-bit register",
                        name
                    ));
                }
                Self::check_suffix(name, suffix_bits, bits)?;
                (xmm_operand(name, *dst)?, SseRm::Reg(*src), bits == 64)
            }
            (SseForm::IntToFloat, Operand::Memory(mem), Operand::Register(dst)) => {
                let bits = suffix_bits.unwrap_or(32);
                (xmm_operand(name, *dst)?, SseRm::Mem(mem), bits == 64)
            }
            (SseForm::FloatToInt, Operand::Register(src), Operand::Register(dst)) => {
                let bits = dst.bits() as u32;
                if *dst == Reg::Rip || !matches!(bits, 32 | 64) {
                    return Err(format!(
                        "{}: integer destination must be a 32-bit or 64-bit register",
                        name
                    ));
                }
                Self::check_suffix(name, suffix_bits, bits)?;
                (*dst, SseRm::Reg(xmm_operand(name, *src)?), bits == 64)
            }
            (SseForm::FloatToInt, Operand::Memory(mem), Operand::Register(dst)) => {
                let bits = dst.bits() as u32;
                if *dst == Reg::Rip || !matches!(bits, 32 | 64) {
                    return Err(format!(
                        "{}: integer destination must be a 32-bit or 64-bit register",
                        name
                    ));
                }
                Self::check_suffix(name, suffix_bits, bits)?;
                (*dst, SseRm::Mem(mem), bits == 64)
            }
            (SseForm::FloatPrecision, Operand::Register(src), Operand::Register(dst)) => (
                xmm_operand(name, *dst)?,
                SseRm::Reg(xmm_operand(name, *src)?),
                false,
            ),
            (SseForm::FloatPrecision, Operand::Memory(mem), Operand::Register(dst)) => {
                (xmm_operand(name, *dst)?, SseRm::Mem(mem), false)
            }
            _ => return Err(format!("unsupported {} operands", name)),
        };

        if let Some(prefix) = spec.prefix {
            self.emit_u8(prefix);
        }

        match rm {
            SseRm::Reg(rm) => {
                self.emit_rex(w, reg.id() >= 8, false, rm.id() >= 8);
                self.emit_u8(0x0F);
                self.emit_u8(spec.opcode);
                self.emit_u8(0b11_000_000 | ((reg.id() & 7) << 3) | (rm.id() & 7));
            }
            SseRm::Mem(mem) => {
                self.emit_rex(
                    w,
                    reg.id() >= 8,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                );
                self.emit_u8(0x0F);
                self.emit_u8(spec.opcode);
                self.emit_memory_modrm(reg.id(), mem)?;
            }
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
    // neg  (F6 /3, F7 /3) / not  (F6 /2, F7 /2)
    //
    // neg: 2の補数による符号反転 (オペランドを `0 - オペランド` にする)。
    // not: 1の補数 (全ビット反転)。フラグは変化しない。
    //
    //   negq %rax        -> REX.W F7 /3
    //   negl -4(%rbp)    -> F7 /3
    //   negb %al         -> F6 /3
    //   notq %rax        -> REX.W F7 /2
    //   notb -1(%rbp)    -> F6 /2
    //
    // - レジスタ: サイズはレジスタ名で決まる。サフィックスが付いていて
    //   レジスタ幅と食い違う場合はエラー。
    // - メモリ  : サイズはサフィックスで決まる。無印は `mov` と
    //   同じく32bit扱い。
    // --------------------------------------------------------
    fn emit_neg(&mut self, operands: &[Operand], suffix_bits: Option<u32>) -> Result<(), String> {
        self.emit_unary_f6f7("neg", 3, operands, suffix_bits)
    }

    fn emit_not(&mut self, operands: &[Operand], suffix_bits: Option<u32>) -> Result<(), String> {
        self.emit_unary_f6f7("not", 2, operands, suffix_bits)
    }

    /// `F6 /digit` (8bit) / `F7 /digit` (16/32/64bit) の1オペランド命令 (neg / not)
    fn emit_unary_f6f7(
        &mut self,
        name: &str,
        digit: u8,
        operands: &[Operand],
        suffix_bits: Option<u32>,
    ) -> Result<(), String> {
        if operands.len() != 1 {
            return Err(format!("{} requires one operand", name));
        }

        match &operands[0] {
            Operand::Register(reg) => {
                let bits = reg.bits() as u32;

                if let Some(suffix) = suffix_bits {
                    if suffix != bits {
                        return Err(format!(
                            "{}: operand size suffix ({} bit) does not match register size ({} bit)",
                            name, suffix, bits
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
                self.emit_u8(0b11_000_000 | (digit << 3) | (reg.id() & 7));

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

                self.emit_memory_modrm(digit, mem)
            }

            _ => Err(format!("invalid {} operand", name)),
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

    // --------------------------------------------------------
    // movzbl / movzbq (0F B6 /r)
    //
    // 8bit値をゼロ拡張して32bit/64bitレジスタへ書き込む。
    // --------------------------------------------------------
    fn emit_movzb(&mut self, operands: &[Operand], dst_bits: u16) -> Result<(), String> {
        let mnemonic = if dst_bits == 64 { "movzbq" } else { "movzbl" };
        if operands.len() != 2 {
            return Err(format!("{} requires two operands", mnemonic));
        }

        let dst = match &operands[1] {
            Operand::Register(reg) if reg.bits() == dst_bits => *reg,
            _ => {
                return Err(format!(
                    "{} destination must be a {}-bit register",
                    mnemonic, dst_bits
                ));
            }
        };

        match &operands[0] {
            Operand::Register(src) => {
                if src.bits() != 8 {
                    return Err(format!("{} source register must be 8-bit", mnemonic));
                }

                // REX.Wで64bit宛先にし、REXの存在によりSPL/BPL/SIL/DILも正しく選ぶ。
                self.emit_rex(dst_bits == 64, dst.id() >= 8, false, src.id() >= 8);
                self.emit_u8(0x0F);
                self.emit_u8(0xB6);
                self.emit_u8(0b11_000_000 | ((dst.id() & 7) << 3) | (src.id() & 7));

                Ok(())
            }

            Operand::Memory(mem) => {
                self.emit_rex(
                    dst_bits == 64,
                    dst.id() >= 8,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                );
                self.emit_u8(0x0F);
                self.emit_u8(0xB6);
                self.emit_memory_modrm(dst.id(), mem)
            }

            _ => Err(format!("invalid {} source operand", mnemonic)),
        }
    }
}

// ============================================================
// and / or / xor / shl / shr / sar
// ============================================================

/// 2オペランドの論理演算 (ALU 命令)
#[derive(Clone, Copy)]
enum AluOp {
    And,
    Or,
    Xor,
}

impl AluOp {
    fn name(self) -> &'static str {
        match self {
            AluOp::And => "and",
            AluOp::Or => "or",
            AluOp::Xor => "xor",
        }
    }

    /// 即値グループ (`80` / `81` / `83` の `/digit`)
    fn group(self) -> u8 {
        match self {
            AluOp::Or => 1,
            AluOp::And => 4,
            AluOp::Xor => 6,
        }
    }

    /// `op r, r/m` (レジスタ -> レジスタ/メモリ) の8bit版オペコード。
    /// 16/32/64bit 版は `+1`、`op r/m, r` (メモリ -> レジスタ) 版は `+2`、
    /// アキュムレータ (`%al` / `%eax` ...) と即値の短縮形は `+4` (8bit) / `+5`。
    fn base_opcode(self) -> u8 {
        match self {
            AluOp::Or => 0x08,
            AluOp::And => 0x20,
            AluOp::Xor => 0x30,
        }
    }
}

/// シフト命令
#[derive(Clone, Copy)]
enum ShiftOp {
    Shl,
    Shr,
    Sar,
}

impl ShiftOp {
    fn name(self) -> &'static str {
        match self {
            ShiftOp::Shl => "shl",
            ShiftOp::Shr => "shr",
            ShiftOp::Sar => "sar",
        }
    }

    /// `C0` / `C1` / `D0` - `D3` の `/digit`
    fn group(self) -> u8 {
        match self {
            ShiftOp::Shl => 4,
            ShiftOp::Shr => 5,
            ShiftOp::Sar => 7,
        }
    }
}

/// シフト量の指定方法
enum ShiftCount {
    /// 1ビット (`shl %eax` / `shl $1, %eax`)
    One,
    /// 即値 (`shl $3, %eax`)
    Imm(u8),
    /// `%cl` (`shl %cl, %eax`)
    Cl,
}

/// 8bit レジスタ `%spl` / `%bpl` / `%sil` / `%dil` は、REX プレフィックスが
/// 無いと `%ah` / `%ch` / `%dh` / `%bh` の意味になるため、空の REX (0x40) が要る
fn needs_empty_rex(reg: Reg) -> bool {
    reg.bits() == 8 && (4..=7).contains(&reg.id())
}

/// 即値がオペランドサイズに収まるか確かめ、符号付きに正規化して返す
/// (`andl $0xFFFFFFFF, %eax` は `-1` と同じ)
fn normalize_imm(name: &str, value: i64, bits: u32) -> Result<i64, String> {
    let ok = match bits {
        8 => (-128..=255).contains(&value),
        16 => (-32768..=65535).contains(&value),
        32 => (i32::MIN as i64..=u32::MAX as i64).contains(&value),
        // 64bit の即値は符号拡張される32bitまで
        64 => (i32::MIN as i64..=i32::MAX as i64).contains(&value),
        _ => false,
    };
    if !ok {
        return Err(format!(
            "{}: immediate {} does not fit in {} bit operand",
            name, value, bits
        ));
    }
    Ok(match bits {
        8 => value as i8 as i64,
        16 => value as i16 as i64,
        32 => value as i32 as i64,
        _ => value,
    })
}

/// `symbol(%rip)` は命令の末尾の即値の分だけ RIP からの距離がずれるが、
/// その再配置は未対応なので弾く (`cmpl $1, symbol(%rip)` と同じ制約)
fn reject_rip_symbol_with_imm(name: &str, mem: &MemoryOperand) -> Result<(), String> {
    if mem.base == Some(Reg::Rip) && mem.symbol.is_some() {
        return Err(format!(
            "{}: symbol(%rip) operand with an immediate is not supported",
            name
        ));
    }
    Ok(())
}

impl Emitter {
    fn emit_inc_dec(
        &mut self,
        operands: &[Operand],
        group: u8,
        suffix_bits: Option<u32>,
    ) -> Result<(), String> {
        let name = if group == 0 { "inc" } else { "dec" };
        if operands.len() != 1 {
            return Err(format!("{} requires one operand", name));
        }

        match &operands[0] {
            Operand::Register(reg) => {
                let bits = reg.bits() as u32;
                if !matches!(bits, 8 | 16 | 32 | 64) {
                    return Err(format!("{}: unsupported operand size", name));
                }
                Self::check_suffix(name, suffix_bits, bits)?;
                self.emit_sized_prefix(bits, false, false, reg.id() >= 8, needs_empty_rex(*reg));
                self.emit_u8(if bits == 8 { 0xFE } else { 0xFF });
                self.emit_u8(0b11_000_000 | (group << 3) | (reg.id() & 7));
                Ok(())
            }
            Operand::Memory(mem) => {
                let bits = suffix_bits
                    .ok_or_else(|| format!("{}: memory operand requires a size suffix", name))?;
                if !matches!(bits, 8 | 16 | 32 | 64) {
                    return Err(format!("{}: unsupported operand size", name));
                }
                self.emit_sized_prefix(
                    bits,
                    false,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                    false,
                );
                self.emit_u8(if bits == 8 { 0xFE } else { 0xFF });
                self.emit_memory_modrm(group, mem)
            }
            operand => Err(format!(
                "{}: expected a register or memory operand, got {:?}",
                name, operand
            )),
        }
    }

    /// `[0x66] [REX]` を出力する。`bits` が 16 なら `0x66`、64 なら REX.W
    fn emit_sized_prefix(&mut self, bits: u32, r: bool, x: bool, b: bool, empty_rex: bool) {
        if bits == 16 {
            self.emit_u8(0x66);
        }
        let rex =
            0x40 | (((bits == 64) as u8) << 3) | ((r as u8) << 2) | ((x as u8) << 1) | (b as u8);
        if rex != 0x40 || empty_rex {
            self.emit_u8(rex);
        }
    }

    /// オペランドサイズ(bit数)が接尾辞と食い違っていないか確かめる
    fn check_suffix(name: &str, suffix_bits: Option<u32>, bits: u32) -> Result<(), String> {
        match suffix_bits {
            Some(suffix) if suffix != bits => Err(format!(
                "{}: operand size suffix ({} bit) does not match register size ({} bit)",
                name, suffix, bits
            )),
            _ => Ok(()),
        }
    }

    // --------------------------------------------------------
    // imul
    //
    //   imulq %rbx             -> REX.W F7 /5          (1オペランド。rdx:rax = rax * r/m)
    //   imul  %rbx, %rax       -> REX.W 0F AF /r       (r/m -> r)
    //   imulq -8(%rbp), %rax   -> REX.W 0F AF /r
    //   imulq $10, %rax        -> REX.W 6B /r ib       (`imulq $10, %rax, %rax` の短縮記法)
    //   imull $1000, %ecx      -> 69 /r id
    //   imulq $10, -8(%rbp), %rax / imul $5, %eax, %ebx
    //                          -> 6B/69 /r             (3オペランド。元は r/m)
    //
    // - サイズは宛先レジスタで決まる (16/32/64bit。8bitのレジスタは2/3オペランド形が無い)。
    //   接尾辞が付いていて宛先の幅と食い違う場合や、元のレジスタの幅が違う場合はエラー。
    // - 即値は `-128..=127` なら `6B` (imm8)、そうでなければ `69`
    //   (16bitはimm16、32/64bitはimm32を符号拡張)。
    // - `symbol(%rip)` + 即値は、他の即値付き命令と同じ理由で未対応。
    // --------------------------------------------------------
    fn emit_imul(&mut self, operands: &[Operand], suffix_bits: Option<u32>) -> Result<(), String> {
        match operands {
            [_] => self.emit_unary_group(operands, 5),

            [Operand::Immediate(value), Operand::Register(dst)] => {
                self.emit_imul_imm(*value, &Operand::Register(*dst), *dst, suffix_bits)
            }
            [
                src @ (Operand::Register(_) | Operand::Memory(_)),
                Operand::Register(dst),
            ] => self.emit_imul_rm(src, *dst, suffix_bits),

            [
                Operand::Immediate(value),
                src @ (Operand::Register(_) | Operand::Memory(_)),
                Operand::Register(dst),
            ] => self.emit_imul_imm(*value, src, *dst, suffix_bits),

            [_, _] | [_, _, _] => Err("imul: invalid operands (destination must be a register, \
                 source must be a register/memory, immediate form needs an immediate)"
                .into()),
            _ => Err("invalid imul operand count".into()),
        }
    }

    /// `imul` の宛先のビット幅を確かめる (8bitは不可、接尾辞と一致が必要)
    fn imul_bits(dst: Reg, suffix_bits: Option<u32>) -> Result<u32, String> {
        let bits = dst.bits() as u32;
        if bits == 8 {
            return Err("imul: 8 bit registers are not supported by the 2/3 operand form".into());
        }
        Self::check_suffix("imul", suffix_bits, bits)?;
        Ok(bits)
    }

    /// `imul r/m, r` (`0F AF /r`)
    fn emit_imul_rm(
        &mut self,
        src: &Operand,
        dst: Reg,
        suffix_bits: Option<u32>,
    ) -> Result<(), String> {
        let bits = Self::imul_bits(dst, suffix_bits)?;
        match src {
            Operand::Register(src) => {
                if src.bits() as u32 != bits {
                    return Err("imul: register size mismatch".into());
                }
                self.emit_sized_prefix(bits, dst.id() >= 8, false, src.id() >= 8, false);
                self.emit_u8(0x0F);
                self.emit_u8(0xAF);
                self.emit_u8(0b11_000_000 | ((dst.id() & 7) << 3) | (src.id() & 7));
                Ok(())
            }
            Operand::Memory(mem) => {
                self.emit_sized_prefix(
                    bits,
                    dst.id() >= 8,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                    false,
                );
                self.emit_u8(0x0F);
                self.emit_u8(0xAF);
                self.emit_memory_modrm(dst.id(), mem)
            }
            _ => Err("imul: invalid source operand".into()),
        }
    }

    /// `imul $imm, r/m, r` (`6B /r ib` / `69 /r iw|id`)
    fn emit_imul_imm(
        &mut self,
        value: i64,
        src: &Operand,
        dst: Reg,
        suffix_bits: Option<u32>,
    ) -> Result<(), String> {
        let bits = Self::imul_bits(dst, suffix_bits)?;
        let value = normalize_imm("imul", value, bits)?;
        let fits_i8 = (-128..=127).contains(&value);

        match src {
            Operand::Register(src) => {
                if src.bits() as u32 != bits {
                    return Err("imul: register size mismatch".into());
                }
                self.emit_sized_prefix(bits, dst.id() >= 8, false, src.id() >= 8, false);
                self.emit_u8(if fits_i8 { 0x6B } else { 0x69 });
                self.emit_u8(0b11_000_000 | ((dst.id() & 7) << 3) | (src.id() & 7));
            }
            Operand::Memory(mem) => {
                reject_rip_symbol_with_imm("imul", mem)?;
                self.emit_sized_prefix(
                    bits,
                    dst.id() >= 8,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                    false,
                );
                self.emit_u8(if fits_i8 { 0x6B } else { 0x69 });
                self.emit_memory_modrm(dst.id(), mem)?;
            }
            _ => return Err("imul: invalid source operand".into()),
        }

        if fits_i8 {
            self.emit_u8(value as i8 as u8);
        } else if bits == 16 {
            self.emit_u16(value as u16);
        } else {
            self.emit_i32(value as i32);
        }
        Ok(())
    }

    // --------------------------------------------------------
    // and / or / xor
    //
    //   andl $0xff, %eax       -> 25 ff 00 00 00   (アキュムレータの短縮形)
    //   orq  $1, %rbx          -> REX.W 83 /1 ib
    //   xor  %edx, %edx        -> 31 d2            (op r, r/m)
    //   and  -4(%rbp), %ecx    -> 23 4d fc         (op r/m, r)
    //   orb  $1, -1(%rbp)      -> 80 /1 ib
    //
    // - レジスタ: サイズはレジスタ名で決まる。接尾辞が付いていて
    //   レジスタ幅と食い違う場合はエラー。
    // - メモリ + 即値: サイズは接尾辞で決まる。無印は `mov` と同じく32bit扱い。
    // --------------------------------------------------------
    fn emit_alu(
        &mut self,
        op: AluOp,
        operands: &[Operand],
        suffix_bits: Option<u32>,
    ) -> Result<(), String> {
        let name = op.name();
        if operands.len() != 2 {
            return Err(format!("{} requires two operands", name));
        }

        let base = op.base_opcode();

        match (&operands[0], &operands[1]) {
            (Operand::Immediate(value), Operand::Register(dst)) => {
                let bits = dst.bits() as u32;
                Self::check_suffix(name, suffix_bits, bits)?;
                let value = normalize_imm(name, *value, bits)?;

                // `%al` / `%ax` / `%eax` / `%rax` は、即値が1byteに収まらない
                // ときだけ、ModRM の無い短縮形 (`op + 4/5`) になる
                // (8bit は常に短縮形。GNU as と同じ選び方)
                let accumulator = dst.id() == 0;
                let fits_i8 = (-128..=127).contains(&value);

                if bits == 8 {
                    if accumulator {
                        self.emit_u8(base + 4);
                        self.emit_u8(value as u8);
                        return Ok(());
                    }
                    self.emit_sized_prefix(
                        bits,
                        false,
                        false,
                        dst.id() >= 8,
                        needs_empty_rex(*dst),
                    );
                    self.emit_u8(0x80);
                    self.emit_u8(0b11_000_000 | (op.group() << 3) | (dst.id() & 7));
                    self.emit_u8(value as u8);
                    return Ok(());
                }

                self.emit_sized_prefix(bits, false, false, dst.id() >= 8, false);
                if accumulator && !fits_i8 {
                    self.emit_u8(base + 5);
                } else {
                    self.emit_u8(if fits_i8 { 0x83 } else { 0x81 });
                    self.emit_u8(0b11_000_000 | (op.group() << 3) | (dst.id() & 7));
                }
                if fits_i8 {
                    self.emit_u8(value as i8 as u8);
                } else if bits == 16 {
                    self.emit_u16(value as u16);
                } else {
                    self.emit_i32(value as i32);
                }
                Ok(())
            }

            (Operand::Immediate(value), Operand::Memory(mem)) => {
                let bits = suffix_bits.unwrap_or(32);
                reject_rip_symbol_with_imm(name, mem)?;
                let value = normalize_imm(name, *value, bits)?;
                self.emit_group_imm_mem(value, mem, op.group(), bits)
            }

            (Operand::Register(src), Operand::Register(dst)) => {
                if src.bits() != dst.bits() {
                    return Err(format!("{}: register size mismatch", name));
                }
                let bits = src.bits() as u32;
                Self::check_suffix(name, suffix_bits, bits)?;

                self.emit_sized_prefix(
                    bits,
                    src.id() >= 8,
                    false,
                    dst.id() >= 8,
                    needs_empty_rex(*src) || needs_empty_rex(*dst),
                );
                self.emit_u8(base + (bits != 8) as u8);
                self.emit_u8(0b11_000_000 | ((src.id() & 7) << 3) | (dst.id() & 7));
                Ok(())
            }

            (Operand::Register(src), Operand::Memory(mem)) => {
                let bits = src.bits() as u32;
                Self::check_suffix(name, suffix_bits, bits)?;

                self.emit_sized_prefix(
                    bits,
                    src.id() >= 8,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                    needs_empty_rex(*src),
                );
                self.emit_u8(base + (bits != 8) as u8);
                self.emit_memory_modrm(src.id(), mem)
            }

            (Operand::Memory(mem), Operand::Register(dst)) => {
                let bits = dst.bits() as u32;
                Self::check_suffix(name, suffix_bits, bits)?;

                self.emit_sized_prefix(
                    bits,
                    dst.id() >= 8,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                    needs_empty_rex(*dst),
                );
                self.emit_u8(base + 2 + (bits != 8) as u8);
                self.emit_memory_modrm(dst.id(), mem)
            }

            (l, r) => Err(format!("unsupported {} operands {:?} {:?}", name, l, r)),
        }
    }

    // --------------------------------------------------------
    // shl / sal / shr / sar
    //
    //   shl  $3, %eax        -> C1 /4 ib
    //   shl  $1, %eax        -> D1 /4        (1bit は短縮形。GNU as と同じ)
    //   shl  %eax            -> D1 /4        (`$1` の省略形)
    //   sarq %cl, %rdx       -> REX.W D3 /7  (シフト量は `%cl` のみ)
    //   shrl $2, -4(%rbp)    -> C1 /5 ib
    //
    // 8bit は `C0` / `D0` / `D2`、それ以外は `C1` / `D1` / `D3`。
    // サイズの決め方は `and` などと同じ (レジスタは名前、メモリは接尾辞)。
    // --------------------------------------------------------
    fn emit_shift(
        &mut self,
        op: ShiftOp,
        operands: &[Operand],
        suffix_bits: Option<u32>,
    ) -> Result<(), String> {
        let name = op.name();

        let (count, dest) = match operands {
            [dest] => (ShiftCount::One, dest),
            [Operand::Immediate(1), dest] => (ShiftCount::One, dest),
            [Operand::Immediate(value), dest] => {
                if !(0..=255).contains(value) {
                    return Err(format!(
                        "{}: shift count must be in 0..=255 (got {})",
                        name, value
                    ));
                }
                (ShiftCount::Imm(*value as u8), dest)
            }
            [Operand::Register(Reg::Cl), dest] => (ShiftCount::Cl, dest),
            [Operand::Register(_), _] => {
                return Err(format!("{}: shift count register must be %cl", name));
            }
            _ => return Err(format!("{} requires one or two operands", name)),
        };

        let bits = match dest {
            Operand::Register(reg) => reg.bits() as u32,
            Operand::Memory(_) => suffix_bits.unwrap_or(32),
            _ => return Err(format!("invalid {} operand", name)),
        };
        if !matches!(bits, 8 | 16 | 32 | 64) {
            return Err(format!("{}: unsupported operand size", name));
        }
        if let Operand::Register(reg) = dest {
            Self::check_suffix(name, suffix_bits, reg.bits() as u32)?;
        }

        let opcode = match (&count, bits == 8) {
            (ShiftCount::One, true) => 0xD0,
            (ShiftCount::One, false) => 0xD1,
            (ShiftCount::Imm(_), true) => 0xC0,
            (ShiftCount::Imm(_), false) => 0xC1,
            (ShiftCount::Cl, true) => 0xD2,
            (ShiftCount::Cl, false) => 0xD3,
        };

        match dest {
            Operand::Register(reg) => {
                self.emit_sized_prefix(bits, false, false, reg.id() >= 8, needs_empty_rex(*reg));
                self.emit_u8(opcode);
                self.emit_u8(0b11_000_000 | (op.group() << 3) | (reg.id() & 7));
            }
            Operand::Memory(mem) => {
                if matches!(count, ShiftCount::Imm(_)) {
                    reject_rip_symbol_with_imm(name, mem)?;
                }
                self.emit_sized_prefix(
                    bits,
                    false,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                    false,
                );
                self.emit_u8(opcode);
                self.emit_memory_modrm(op.group(), mem)?;
            }
            _ => unreachable!(),
        }

        if let ShiftCount::Imm(value) = count {
            self.emit_u8(value);
        }

        Ok(())
    }
}

#[cfg(test)]
mod sse_tests {
    use crate::assembler::x64::{Lexer, Parser, convert};

    use super::*;

    fn assemble(source: &str) -> Result<Vec<u8>, String> {
        let tokens = Lexer::new(source).tokenize()?;
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
            ("cvtsi2ss %eax, %xmm0", "f30f2ac0"),
            ("cvtsi2ssq %rax, %xmm0", "f3480f2ac0"),
            ("cvtsi2ss (%rax), %xmm0", "f30f2a00"),
            ("cvtsi2sd %r9, %xmm10", "f24d0f2ad1"),
            ("cvtsi2ssl -4(%rbp), %xmm1", "f30f2a4dfc"),
            ("cvtsi2sdq 8(%r12), %xmm8", "f24d0f2a442408"),
            ("cvttss2si %xmm0, %eax", "f30f2cc0"),
            ("cvttss2siq %xmm10, %r9", "f34d0f2cca"),
            ("cvttsd2si %xmm1, %rax", "f2480f2cc1"),
            ("cvttsd2sil -8(%rbp), %r8d", "f2440f2c45f8"),
            ("cvttsd2siq 16(%r13,%r12,4), %rax", "f24b0f2c44a510"),
            ("cvtss2sd %xmm1, %xmm0", "f30f5ac1"),
            ("cvtss2sd 4(%rax), %xmm9", "f3440f5a4804"),
            ("cvtsd2ss %xmm10, %xmm2", "f2410f5ad2"),
            ("cvtsd2ss -8(%rbp), %xmm8", "f2440f5a45f8"),
        ];

        for (asm, expected) in cases {
            let bytes = assemble(asm).unwrap_or_else(|e| panic!("{}: {}", asm, e));
            assert_eq!(hex(&bytes), *expected, "{}", asm);
        }
    }

    #[test]
    fn sse_rejects_invalid_operands() {
        let cases = [
            "addss %ymm1, %ymm0",         // VEX が必要
            "addps %zmm1, %zmm0",         // EVEX が必要
            "addss %eax, %xmm0",          // 汎用レジスタ
            "addss %xmm0, -4(%rbp)",      // addss にストア形式はない
            "cmpps $8, %xmm1, %xmm0",     // 述語は 0..=7
            "cmpps %xmm1, %xmm0",         // imm8 がない
            "cmpps $1, foo(%rip), %xmm0", // imm8 付き RIP 相対シンボルは未対応
            "movss %xmm0",                // オペランド不足
            "add %xmm0, %xmm1",           // SSE 以外の命令はベクトルレジスタ不可
            "mov %xmm0, %rax",
            "cvtsi2ss %xmm0, %xmm1",  // integer source required
            "cvtsi2ss %rax, %eax",    // xmm destination required
            "cvtsi2ssq %eax, %xmm0",  // suffix/register width mismatch
            "cvttss2si %xmm0, %xmm1", // integer destination required
            "cvttsd2siq %xmm0, %eax", // suffix/register width mismatch
            "cvtss2sd %xmm0, %eax",   // xmm destination required
            "cvtsd2ss %ymm0, %xmm0",  // VEX source is unsupported
        ];

        for asm in cases {
            assert!(assemble(asm).is_err(), "{} should fail", asm);
        }
    }
}

#[cfg(test)]
mod alu_shift_tests {
    use crate::assembler::x64::{Lexer, Parser, convert};

    use super::*;

    fn assemble(source: &str) -> Result<Vec<u8>, String> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::new(tokens).parse()?;
        let program = convert::convert_program(&ast)?;
        let mut emitter = Emitter::new();
        emitter.emit_program(&program)?;
        Ok(emitter.text)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// GNU as (binutils) の出力と一致するか。期待値は `as` + `objcopy` で生成したもの
    #[test]
    fn and_or_xor_shift_match_gnu_as() {
        let cases: &[(&str, &str)] = &[
            ("and %ecx, %eax", "21c8"),
            ("and %rcx, %rax", "4821c8"),
            ("and %cx, %ax", "6621c8"),
            ("and %cl, %al", "20c8"),
            ("and %sil, %dil", "4020f7"),
            ("and %r9b, %al", "4420c8"),
            ("and %al, %r10b", "4120c2"),
            ("and %r8, %r15", "4d21c7"),
            ("and %r8d, %ecx", "4421c1"),
            ("and %cx, %r9w", "664121c9"),
            ("and -4(%rbp), %eax", "2345fc"),
            ("and -8(%rbp), %r9", "4c234df8"),
            ("and 8(%r12,%r13,4), %r10d", "472354ac08"),
            ("and %eax, -4(%rbp)", "2145fc"),
            ("and %r11, (%rsp)", "4c211c24"),
            ("and %sil, -1(%rbp)", "402075ff"),
            ("and -1(%rbp), %dil", "40227dff"),
            ("andw %cx, 2(%rax)", "66214802"),
            ("and $1, %eax", "83e001"),
            ("and $255, %eax", "25ff000000"),
            ("and $0xff, %ecx", "81e1ff000000"),
            ("and $0x1234, %eax", "2534120000"),
            ("and $0x12345678, %eax", "2578563412"),
            ("and $0x12345678, %ebx", "81e378563412"),
            ("and $-1, %rax", "4883e0ff"),
            ("and $0x7fffffff, %rax", "4825ffffff7f"),
            ("and $-2, %rbx", "4883e3fe"),
            ("and $0x12345678, %r9", "4981e178563412"),
            ("and $15, %al", "240f"),
            ("and $15, %bl", "80e30f"),
            ("and $15, %sil", "4080e60f"),
            ("and $15, %r9b", "4180e10f"),
            ("and $1, %ax", "6683e001"),
            ("and $0x1234, %ax", "66253412"),
            ("and $0x1234, %cx", "6681e13412"),
            ("and $0xffff, %ax", "6683e0ff"),
            ("and $0xffffffff, %eax", "83e0ff"),
            ("andl $1, -4(%rbp)", "8365fc01"),
            ("andl $0x1000, -4(%rbp)", "8165fc00100000"),
            ("andq $7, -8(%rbp)", "488365f807"),
            ("andq $0x12345678, (%rax)", "48812078563412"),
            ("andw $3, 2(%rax)", "6683600203"),
            ("andw $0x1234, 2(%rax)", "668160023412"),
            ("andb $1, -1(%rbp)", "8065ff01"),
            ("andb $0xff, (%rdi)", "8027ff"),
            ("andl $1, 8(%r12,%r13,4)", "438364ac0801"),
            ("andq %rax, %rbx", "4821c3"),
            ("or %ecx, %eax", "09c8"),
            ("or %rcx, %rax", "4809c8"),
            ("or %cl, %al", "08c8"),
            ("or %r9d, %r10d", "4509ca"),
            ("or -4(%rbp), %eax", "0b45fc"),
            ("or %eax, -4(%rbp)", "0945fc"),
            ("or $1, %eax", "83c801"),
            ("or $0x12345678, %eax", "0d78563412"),
            ("or $0x12345678, %edx", "81ca78563412"),
            ("or $-1, %rcx", "4883c9ff"),
            ("or $1, %al", "0c01"),
            ("or $1, %cl", "80c901"),
            ("orl $1, -4(%rbp)", "834dfc01"),
            ("orb $0x80, -1(%rbp)", "804dff80"),
            ("orq $0x12345678, 16(%rax)", "4881481078563412"),
            ("orw $1, (%rax)", "66830801"),
            ("xor %eax, %eax", "31c0"),
            ("xor %edx, %edx", "31d2"),
            ("xor %r8d, %r8d", "4531c0"),
            ("xor %rax, %rax", "4831c0"),
            ("xor %rdx, %rdx", "4831d2"),
            ("xor %cx, %cx", "6631c9"),
            ("xor %cl, %cl", "30c9"),
            ("xor %esi, %edi", "31f7"),
            ("xor -4(%rbp), %ecx", "334dfc"),
            ("xor %ecx, -4(%rbp)", "314dfc"),
            ("xor %r12, 8(%rbp)", "4c316508"),
            ("xor $1, %eax", "83f001"),
            ("xor $0x12345678, %eax", "3578563412"),
            ("xor $-1, %ebx", "83f3ff"),
            ("xor $0x55, %al", "3455"),
            ("xor $0x55, %dl", "80f255"),
            ("xorl $1, -4(%rbp)", "8375fc01"),
            ("xorb $1, -1(%rbp)", "8075ff01"),
            ("xorq $-1, -8(%rbp)", "488375f8ff"),
            ("xorw $0x1234, -2(%rbp)", "668175fe3412"),
            ("shl $3, %eax", "c1e003"),
            ("shl $1, %eax", "d1e0"),
            ("shl %eax", "d1e0"),
            ("shl $0, %eax", "c1e000"),
            ("shl $31, %r9d", "41c1e11f"),
            ("shl $63, %rax", "48c1e03f"),
            ("shl $255, %eax", "c1e0ff"),
            ("shl $4, %ax", "66c1e004"),
            ("shl $1, %ax", "66d1e0"),
            ("shl $2, %al", "c0e002"),
            ("shl $1, %al", "d0e0"),
            ("shl $2, %sil", "40c0e602"),
            ("shl $2, %r10b", "41c0e202"),
            ("shl $5, %r9", "49c1e105"),
            ("shl %cl, %eax", "d3e0"),
            ("shl %cl, %rax", "48d3e0"),
            ("shl %cl, %ax", "66d3e0"),
            ("shl %cl, %al", "d2e0"),
            ("shl %cl, %r11d", "41d3e3"),
            ("shl %cl, %sil", "40d2e6"),
            ("shll $2, -4(%rbp)", "c165fc02"),
            ("shll $1, -4(%rbp)", "d165fc"),
            ("shll -4(%rbp)", "d165fc"),
            ("shlq $3, -8(%rbp)", "48c165f803"),
            ("shlw $3, 2(%rax)", "66c1600203"),
            ("shlb $3, -1(%rbp)", "c065ff03"),
            ("shlb $1, -1(%rbp)", "d065ff"),
            ("shll %cl, -4(%rbp)", "d365fc"),
            ("shlq %cl, (%rax)", "48d320"),
            ("shlb %cl, (%rdi)", "d227"),
            ("shlw %cl, (%rax)", "66d320"),
            ("shll $2, 8(%r12,%r13,4)", "43c164ac0802"),
            ("sal $3, %eax", "c1e003"),
            ("sal %cl, %rax", "48d3e0"),
            ("sall $2, -4(%rbp)", "c165fc02"),
            ("shr $3, %eax", "c1e803"),
            ("shr $1, %rdx", "48d1ea"),
            ("shr %rdx", "48d1ea"),
            ("shr $2, %r8", "49c1e802"),
            ("shr %cl, %edx", "d3ea"),
            ("shr %cl, %r8b", "41d2e8"),
            ("shrl $5, -4(%rbp)", "c16dfc05"),
            ("shrq %cl, -8(%rbp)", "48d36df8"),
            ("shrb $1, (%rax)", "d028"),
            ("shrw $7, (%rbx)", "66c12b07"),
            ("sar $3, %eax", "c1f803"),
            ("sar $1, %eax", "d1f8"),
            ("sar $63, %rax", "48c1f83f"),
            ("sar %cl, %eax", "d3f8"),
            ("sar %cl, %r9", "49d3f9"),
            ("sarl $2, -4(%rbp)", "c17dfc02"),
            ("sarq %cl, -8(%rbp)", "48d37df8"),
            ("sarb $4, -1(%rbp)", "c07dff04"),
            ("sarw $1, -2(%rbp)", "66d17dfe"),
            ("sarq $63, 16(%r9)", "49c179103f"),
        ];

        for (asm, expected) in cases {
            let bytes = assemble(asm).unwrap_or_else(|e| panic!("{}: {}", asm, e));
            assert_eq!(hex(&bytes), *expected, "{}", asm);
        }
    }

    #[test]
    fn and_or_xor_shift_reject_invalid_operands() {
        let cases = [
            "and %eax, %rbx",        // レジスタのサイズ不一致
            "andq %eax, %ebx",       // 接尾辞とレジスタ幅が不一致
            "or $0x100000000, %eax", // 即値が32bitに収まらない
            "or $0x100, %al",        // 即値が8bitに収まらない
            "xor $0x80000000, %rax", // 64bit の即値は符号付き32bitまで
            "and $1, foo(%rip)",     // 即値付きの RIP 相対シンボルは未対応
            "and %xmm0, %xmm1",      // ベクトルレジスタ
            "and %eax",              // オペランド不足
            "shl %al, %eax",         // シフト量のレジスタは %cl のみ
            "shl %ecx, %eax",        // %cl 以外
            "shl $256, %eax",        // シフト量は 0..=255
            "shl $-1, %eax",
            "shlq $1, %eax",      // 接尾辞とレジスタ幅が不一致
            "shl $1, $2",         // 宛先が即値
            "shl $1, %eax, %ebx", // オペランド過多
            "shl $2, foo(%rip)",  // 即値付きの RIP 相対シンボルは未対応
        ];

        for asm in cases {
            assert!(assemble(asm).is_err(), "{} should fail", asm);
        }
    }
}

#[cfg(test)]
mod inc_dec_tests {
    use crate::assembler::x64::{Lexer, Parser, convert};

    use super::*;

    fn assemble(source: &str) -> Result<Vec<u8>, String> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::new(tokens).parse()?;
        let program = convert::convert_program(&ast)?;
        let mut emitter = Emitter::new();
        emitter.emit_program(&program)?;
        Ok(emitter.text)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn inc_dec_encode_register_and_memory_widths() {
        let cases: &[(&str, &str)] = &[
            ("incb %al", "fec0"),
            ("incb %spl", "40fec4"),
            ("incw %ax", "66ffc0"),
            ("incl %eax", "ffc0"),
            ("incq %rax", "48ffc0"),
            ("incl %r9d", "41ffc1"),
            ("incq %r9", "49ffc1"),
            ("incb -1(%rbp)", "fe45ff"),
            ("incw 2(%rax)", "66ff4002"),
            ("incl 8(%r12)", "41ff442408"),
            ("incq -8(%rbp)", "48ff45f8"),
            ("decb %spl", "40fecc"),
            ("decw %r8w", "6641ffc8"),
            ("decl %r9d", "41ffc9"),
            ("decq %r9", "49ffc9"),
            ("decb -1(%rbp)", "fe4dff"),
            ("decq -8(%rbp)", "48ff4df8"),
        ];

        for (asm, expected) in cases {
            let bytes = assemble(asm).unwrap_or_else(|err| panic!("{}: {}", asm, err));
            assert_eq!(hex(&bytes), *expected, "{}", asm);
        }
    }

    #[test]
    fn inc_dec_reject_invalid_operands() {
        for asm in [
            "inc",
            "dec %eax, %ebx",
            "incq %eax",
            "decb %ax",
            "inc (%rax)",
            "inc $1",
            "dec %xmm0",
        ] {
            assert!(assemble(asm).is_err(), "{} should fail", asm);
        }
    }
}

#[cfg(test)]
mod not_tests {
    use crate::assembler::x64::{Lexer, Parser, convert};

    use super::*;

    fn assemble(source: &str) -> Result<Vec<u8>, String> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::new(tokens).parse()?;
        let program = convert::convert_program(&ast)?;
        let mut emitter = Emitter::new();
        emitter.emit_program(&program)?;
        Ok(emitter.text)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// 期待値は GNU as の出力
    #[test]
    fn not_encodes_register_and_memory_widths() {
        let cases: &[(&str, &str)] = &[
            ("notb %al", "f6d0"),
            ("notb %spl", "40f6d4"),
            ("notw %ax", "66f7d0"),
            ("notl %eax", "f7d0"),
            ("notq %rax", "48f7d0"),
            ("not %rbx", "48f7d3"),
            ("notl %r9d", "41f7d1"),
            ("notq %r9", "49f7d1"),
            ("notw %r8w", "6641f7d0"),
            ("notb -1(%rbp)", "f655ff"),
            ("notw 2(%rax)", "66f75002"),
            ("notl 8(%r12)", "41f7542408"),
            ("notq -8(%rbp)", "48f755f8"),
            ("notq 8(%rsp,%r9,4)", "4af7548c08"),
        ];

        for (asm, expected) in cases {
            let bytes = assemble(asm).unwrap_or_else(|err| panic!("{}: {}", asm, err));
            assert_eq!(hex(&bytes), *expected, "{}", asm);
        }
    }

    #[test]
    fn not_rejects_invalid_operands() {
        for asm in [
            "not",
            "not %eax, %ebx",
            "notq %eax",
            "notb %ax",
            "not $1",
            "not %xmm0",
        ] {
            assert!(assemble(asm).is_err(), "{} should fail", asm);
        }
    }
}

#[cfg(test)]
mod imul_tests {
    use crate::assembler::x64::{Lexer, Parser, convert};

    use super::*;

    fn assemble(source: &str) -> Result<Vec<u8>, String> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::new(tokens).parse()?;
        let program = convert::convert_program(&ast)?;
        let mut emitter = Emitter::new();
        emitter.emit_program(&program)?;
        Ok(emitter.text)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// 期待値は GNU as の出力
    #[test]
    fn imul_encodes_immediate_memory_and_register_forms() {
        let cases: &[(&str, &str)] = &[
            ("imulq $10, %rax", "486bc00a"),
            ("imul $10, %rax", "486bc00a"),
            ("imull $1000, %ecx", "69c9e8030000"),
            ("imulw $3, %ax", "666bc003"),
            ("imulw $1000, %bx", "6669dbe803"),
            ("imulq $-1, %r9", "4d6bc9ff"),
            ("imulq $100000, %r9, %r10", "4d69d1a0860100"),
            ("imul $5, %eax, %ebx", "6bd805"),
            ("imulq -8(%rbp), %rax", "480faf45f8"),
            ("imull 4(%rdx), %r9d", "440faf4a04"),
            ("imulq $10, -8(%rbp), %rax", "486b45f80a"),
            ("imul %rbx, %rax", "480fafc3"),
            ("imulw %cx, %dx", "660fafd1"),
            ("imulq $10, 8(%rsp,%r9,4), %r11", "4e6b5c8c080a"),
            ("imulq %rbx", "48f7eb"),
            ("imull -4(%rbp), %ecx", "0faf4dfc"),
            ("imulw $1000, 2(%rax), %bx", "66695802e803"),
        ];

        for (asm, expected) in cases {
            let bytes = assemble(asm).unwrap_or_else(|err| panic!("{}: {}", asm, err));
            assert_eq!(hex(&bytes), *expected, "{}", asm);
        }
    }

    #[test]
    fn imul_rejects_invalid_operands() {
        for asm in [
            "imul",
            "imul %al, %bl",
            "imulq %eax, %ebx",
            "imulq %rax, %ebx",
            "imul $10, $5",
            "imul $10",
            "imul %rax, -8(%rbp)",
            "imul $0x1ffffffff, %rax",
            "imulq $1, foo(%rip), %rax",
            "imul $1, %rax, %rbx, %rcx",
        ] {
            assert!(assemble(asm).is_err(), "{} should fail", asm);
        }
    }
}

#[cfg(test)]
mod jcc_tests {
    use crate::assembler::x64::{Lexer, Parser, convert};

    use super::*;

    fn assemble(source: &str) -> Result<Vec<u8>, String> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::new(tokens).parse()?;
        let program = convert::convert_program(&ast)?;
        let mut emitter = Emitter::new();
        emitter.emit_program(&program)?;
        // ラベルへのジャンプ (`jcc` / `jrcxz`) の相対値は `finish` で確定する
        emitter.finish()?;
        Ok(emitter.text)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// 16種類の条件と別名すべてが、GNU as の `jcc rel32` (`0F 8x`) と一致するか。
    /// 期待値は `as` + `objcopy` で、ターゲットを 127 バイトより先に置いて生成したもの
    /// (近いと `as` が短い形 `7x` に最適化するため)。ターゲット直後のラベルなので rel32 は 0
    #[test]
    fn all_jcc_match_gnu_as() {
        let cases: &[(&str, &str)] = &[
            ("jo", "0f8000000000"),
            ("jno", "0f8100000000"),
            ("jb", "0f8200000000"),
            ("jc", "0f8200000000"),
            ("jnae", "0f8200000000"),
            ("jae", "0f8300000000"),
            ("jnb", "0f8300000000"),
            ("jnc", "0f8300000000"),
            ("je", "0f8400000000"),
            ("jz", "0f8400000000"),
            ("jne", "0f8500000000"),
            ("jnz", "0f8500000000"),
            ("jbe", "0f8600000000"),
            ("jna", "0f8600000000"),
            ("ja", "0f8700000000"),
            ("jnbe", "0f8700000000"),
            ("js", "0f8800000000"),
            ("jns", "0f8900000000"),
            ("jp", "0f8a00000000"),
            ("jpe", "0f8a00000000"),
            ("jnp", "0f8b00000000"),
            ("jpo", "0f8b00000000"),
            ("jl", "0f8c00000000"),
            ("jnge", "0f8c00000000"),
            ("jge", "0f8d00000000"),
            ("jnl", "0f8d00000000"),
            ("jle", "0f8e00000000"),
            ("jng", "0f8e00000000"),
            ("jg", "0f8f00000000"),
            ("jnle", "0f8f00000000"),
        ];
        for (mnemonic, expected) in cases {
            let bytes = assemble(&format!("{} L\nL:\n", mnemonic)).unwrap();
            assert_eq!(hex(&bytes), *expected, "{}", mnemonic);
        }
    }

    /// 後ろ向きのジャンプ (rel32 が負)
    #[test]
    fn jcc_backward_target() {
        // L: / jne L → 0F 85 と rel32 = -6
        let bytes = assemble("L:\njne L\n").unwrap();
        assert_eq!(hex(&bytes), "0f85faffffff");
    }

    #[test]
    fn jrcxz_jecxz_match_gnu_as() {
        assert_eq!(hex(&assemble("jrcxz L\nL:\n").unwrap()), "e300");
        assert_eq!(hex(&assemble("jecxz L\nL:\n").unwrap()), "67e300");
    }

    /// rel8 は ±127 バイトまで。範囲を超えたらエラー
    #[test]
    fn jrcxz_rel8_range() {
        let near = format!("jrcxz L\n{}L:\n", "nop\n".repeat(127));
        assert_eq!(&hex(&assemble(&near).unwrap())[..4], "e37f");

        let far = format!("jrcxz L\n{}L:\n", "nop\n".repeat(128));
        assert!(assemble(&far).unwrap_err().contains("rel8 out of range"));
    }

    #[test]
    fn jcc_requires_symbol_operand() {
        assert!(assemble("jo $1\n").is_err());
        assert!(assemble("jrcxz $1\n").is_err());
        // Jcc ではないニーモニックは対象外 (`jcxz` は64bitモードに無い)
        assert!(
            assemble("jcxz L\nL:\n")
                .unwrap_err()
                .contains("unsupported instruction")
        );
    }
}

#[cfg(test)]
mod cmov_tests {
    use crate::assembler::x64::{Lexer, Parser, convert};

    use super::*;

    fn assemble(source: &str) -> Result<Vec<u8>, String> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::new(tokens).parse()?;
        let program = convert::convert_program(&ast)?;
        let mut emitter = Emitter::new();
        emitter.emit_program(&program)?;
        emitter.finish()?;
        Ok(emitter.text)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn cmov_encodes_register_and_memory_operands() {
        let cases: &[(&str, &str)] = &[
            ("cmovl %eax, %ecx", "0f4cc8"),
            ("cmovge %cx, %ax", "660f4dc1"),
            ("cmovne %r8, %r9", "4d0f45c8"),
            ("cmovb %eax, %edx", "0f42d0"),
            ("cmovnbl %eax, %ecx", "0f43c8"),
            ("cmovlew %ax, %cx", "660f4ec8"),
            ("cmovle 8(%r12), %r9", "4d0f4e4c2408"),
        ];

        for (source, expected) in cases {
            assert_eq!(
                hex(&assemble(&format!("{}\n", source)).unwrap()),
                *expected,
                "{}",
                source
            );
        }
    }

    #[test]
    fn cmov_supports_all_condition_codes_and_aliases() {
        let names = [
            "o", "no", "b", "c", "nae", "ae", "nb", "nc", "e", "z", "ne", "nz", "be", "na", "a",
            "nbe", "s", "ns", "p", "pe", "np", "po", "l", "nge", "ge", "nl", "le", "ng", "g",
            "nle",
        ];
        let codes = [
            0x40, 0x41, 0x42, 0x42, 0x42, 0x43, 0x43, 0x43, 0x44, 0x44, 0x45, 0x45, 0x46, 0x46,
            0x47, 0x47, 0x48, 0x49, 0x4a, 0x4a, 0x4b, 0x4b, 0x4c, 0x4c, 0x4d, 0x4d, 0x4e, 0x4e,
            0x4f, 0x4f,
        ];

        for (name, code) in names.iter().zip(codes) {
            let source = format!("cmov{} %eax, %ecx\n", name);
            let expected = format!("0f{:02x}c8", code);
            assert_eq!(hex(&assemble(&source).unwrap()), expected, "{}", source);
        }
    }

    #[test]
    fn cmov_rejects_invalid_operands_and_sizes() {
        for source in [
            "cmovl $1, %eax",
            "cmovl %eax, (%rax)",
            "cmovl %ax, %ecx",
            "cmovb %al, %cl",
            "cmovlq %eax, %ecx",
            "cmovl %eax",
        ] {
            assert!(assemble(&format!("{}\n", source)).is_err(), "{}", source);
        }
    }
}

#[cfg(test)]
mod setcc_tests {
    use crate::assembler::x64::{Lexer, Parser, convert};

    use super::*;

    fn assemble(source: &str) -> Result<Vec<u8>, String> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::new(tokens).parse()?;
        let program = convert::convert_program(&ast)?;
        let mut emitter = Emitter::new();
        emitter.emit_program(&program)?;
        emitter.finish()?;
        Ok(emitter.text)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// 期待値は GNU as (`as` + `objdump -d`) の出力
    #[test]
    fn setcc_encodes_register_and_memory_operands() {
        let cases: &[(&str, &str)] = &[
            ("sete %al", "0f94c0"),
            ("setl %cl", "0f9cc1"),
            ("setg %dl", "0f9fc2"),
            ("setb %sil", "400f92c6"),
            ("setne %r8b", "410f95c0"),
            ("sete %r15b", "410f94c7"),
            ("setle (%rax)", "0f9e00"),
            ("setae 8(%r12)", "410f93442408"),
            ("seta %dil", "400f97c7"),
            ("setnae %bl", "0f92c3"),
            ("setz -4(%rbp)", "0f9445fc"),
            ("setp %spl", "400f9ac4"),
            ("sets %bpl", "400f98c5"),
            ("setnle (%rax,%rcx,4)", "0f9f0488"),
        ];

        for (source, expected) in cases {
            assert_eq!(
                hex(&assemble(&format!("{}\n", source)).unwrap()),
                *expected,
                "{}",
                source
            );
        }
    }

    #[test]
    fn setcc_supports_all_condition_codes_and_aliases() {
        let names = [
            "o", "no", "b", "c", "nae", "ae", "nb", "nc", "e", "z", "ne", "nz", "be", "na", "a",
            "nbe", "s", "ns", "p", "pe", "np", "po", "l", "nge", "ge", "nl", "le", "ng", "g",
            "nle",
        ];
        let codes = [
            0x90, 0x91, 0x92, 0x92, 0x92, 0x93, 0x93, 0x93, 0x94, 0x94, 0x95, 0x95, 0x96, 0x96,
            0x97, 0x97, 0x98, 0x99, 0x9a, 0x9a, 0x9b, 0x9b, 0x9c, 0x9c, 0x9d, 0x9d, 0x9e, 0x9e,
            0x9f, 0x9f,
        ];

        for (name, code) in names.iter().zip(codes) {
            let source = format!("set{} %al\n", name);
            let expected = format!("0f{:02x}c0", code);
            assert_eq!(hex(&assemble(&source).unwrap()), expected, "{}", source);
        }
    }

    #[test]
    fn setcc_rejects_invalid_operands() {
        for source in [
            "sete %eax",
            "sete %ax",
            "sete %rax",
            "sete $1",
            "sete",
            "sete %al, %cl",
            "sete %xmm0",
            "setq %al",
            "setl %eax",
        ] {
            assert!(assemble(&format!("{}\n", source)).is_err(), "{}", source);
        }
    }
}

#[cfg(test)]
mod string_rep_tests {
    use crate::assembler::x64::{Lexer, Parser, convert};

    use super::*;

    fn assemble(source: &str) -> Result<Vec<u8>, String> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::new(tokens).parse()?;
        let program = convert::convert_program(&ast)?;
        let mut emitter = Emitter::new();
        emitter.emit_program(&program)?;
        emitter.finish()?;
        Ok(emitter.text)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn check(cases: &[(&str, &str)]) {
        for (source, expected) in cases {
            assert_eq!(
                hex(&assemble(&format!("{}\n", source)).unwrap()),
                *expected,
                "{}",
                source
            );
        }
    }

    /// 期待値はすべて GNU as (`as` + `objdump -d`) の出力
    #[test]
    fn string_instructions_encode_each_operand_size() {
        check(&[
            ("movsb", "a4"),
            ("movsw", "66a5"),
            ("movsl", "a5"),
            ("movsq", "48a5"),
            ("stosb", "aa"),
            ("stosw", "66ab"),
            ("stosl", "ab"),
            ("stosq", "48ab"),
            ("lodsb", "ac"),
            ("lodsw", "66ad"),
            ("lodsl", "ad"),
            ("lodsq", "48ad"),
            ("scasb", "ae"),
            ("scasw", "66af"),
            ("scasl", "af"),
            ("scasq", "48af"),
            ("cmpsb", "a6"),
            ("cmpsw", "66a7"),
            ("cmpsl", "a7"),
            ("cmpsq", "48a7"),
            ("movsd", "a5"),
            ("cmpsd", "a7"),
        ]);
    }

    #[test]
    fn rep_prefixes_encode_before_rex_and_after_operand_size() {
        check(&[
            ("rep movsb", "f3a4"),
            ("rep movsw", "66f3a5"),
            ("rep movsl", "f3a5"),
            ("rep movsq", "f348a5"),
            ("rep stosb", "f3aa"),
            ("rep stosw", "66f3ab"),
            ("rep stosl", "f3ab"),
            ("rep stosq", "f348ab"),
            ("rep lodsb", "f3ac"),
            ("rep lodsq", "f348ad"),
            ("repe cmpsb", "f3a6"),
            ("repz cmpsl", "f3a7"),
            ("repe cmpsw", "66f3a7"),
            ("repe cmpsq", "f348a7"),
            ("repne scasb", "f2ae"),
            ("repnz scasw", "66f2af"),
            ("repne scasl", "f2af"),
            ("repne scasq", "f248af"),
            ("repe scasb", "f3ae"),
            ("rep scasb", "f3ae"),
            ("rep cmpsb", "f3a6"),
            ("repe movsb", "f3a4"),
            ("repne movsb", "f2a4"),
            ("repne cmpsb", "f2a6"),
            ("rep movsd", "f3a5"),
            ("repne cmpsd", "f2a7"),
        ]);
    }

    #[test]
    fn rep_is_case_insensitive_and_takes_separate_lines() {
        check(&[("REP MOVSB", "f3a4"), ("RepNe ScasB", "f2ae")]);
        assert_eq!(
            hex(&assemble("rep movsb\nrep stosq\n").unwrap()),
            "f3a4f348ab"
        );
    }

    #[test]
    fn cld_and_std_encode() {
        check(&[("cld", "fc"), ("std", "fd")]);
    }

    #[test]
    fn test_encodes_register_immediate_and_memory_forms() {
        check(&[
            ("testb %dl, %dl", "84d2"),
            ("testb %cl, %dl", "84ca"),
            ("testl %eax, %eax", "85c0"),
            ("testq %rax, %rcx", "4885c1"),
            ("testw %ax, %cx", "6685c1"),
            ("test %al, %al", "84c0"),
            ("test %eax, %ebx", "85c3"),
            ("test %rax, %rax", "4885c0"),
            ("testq %r8, %r9", "4d85c1"),
            ("testl %r9d, %r10d", "4585ca"),
            ("testb %spl, %dil", "4084e7"),
            ("testb %sil, %al", "4084f0"),
            ("testb %r8b, %r9b", "4584c1"),
            ("testb $1, %al", "a801"),
            ("testb $1, %cl", "f6c101"),
            ("testb $0x80, %sil", "40f6c680"),
            ("testb $0xff, %al", "a8ff"),
            ("testb $-1, %dl", "f6c2ff"),
            ("testb $1, %r8b", "41f6c001"),
            ("testl $1, %eax", "a901000000"),
            ("testl $0x1000, %eax", "a900100000"),
            ("testl $1, %ecx", "f7c101000000"),
            ("testq $1, %rax", "48a901000000"),
            ("testq $0x12345678, %rcx", "48f7c178563412"),
            ("testq $-1, %rax", "48a9ffffffff"),
            ("testq $1, %r9", "49f7c101000000"),
            ("testw $1, %ax", "66a90100"),
            ("testw $0x1234, %cx", "66f7c13412"),
            ("testl $1, %r8d", "41f7c001000000"),
            ("test $1, %eax", "a901000000"),
            ("test $0xff, %cl", "f6c1ff"),
            ("testb $1, -1(%rbp)", "f645ff01"),
            ("testl $1, -4(%rbp)", "f745fc01000000"),
            ("testq $1, -8(%rbp)", "48f745f801000000"),
            ("testw $1, -2(%rbp)", "66f745fe0100"),
            ("testl $0x1000, (%rax)", "f70000100000"),
            ("testb $1, 8(%r12)", "41f644240801"),
            ("testl $1, (%rax,%rcx,4)", "f7048801000000"),
            ("testl %eax, -4(%rbp)", "8545fc"),
            ("testb %al, -1(%rbp)", "8445ff"),
            ("testb %sil, -1(%rbp)", "408475ff"),
            ("testq %r8, -8(%rbp)", "4c8545f8"),
            ("testw %ax, (%rdx)", "668502"),
            ("testl %eax, 8(%r9)", "41854108"),
            ("testl 8(%r9), %eax", "41854108"),
            ("testb -1(%rbp), %cl", "844dff"),
            ("testq (%rax,%rcx,8), %rdx", "488514c8"),
        ]);
    }

    /// `file.hexl` のインラインasm (os_copy / os_find / Windows の ReadFile 後処理) と同じ並び
    #[test]
    fn library_asm_blocks_match_gnu_as() {
        let copy = "movq -8(%rbp), %rax\nmovq -16(%rbp), %rdx\nmovq -24(%rbp), %rcx\n\
                    pushq %rsi\npushq %rdi\nmovq %rax, %rdi\nmovq %rdx, %rsi\ncld\n\
                    rep movsb\npopq %rdi\npopq %rsi\n";
        assert_eq!(
            hex(&assemble(copy).unwrap()),
            "488b45f8488b55f0488b4de856574889c74889d6fcf3a45f5e"
        );

        let find = "movq -8(%rbp), %rdx\nmovq -16(%rbp), %rcx\nmovq -24(%rbp), %rax\n\
                    movq %rcx, %r8\nmovq %rcx, %r9\npushq %rdi\nmovq %rdx, %rdi\ncld\n\
                    repne scasb\nsete %dl\npopq %rdi\nmovq %r8, %rax\nsubq %rcx, %rax\n\
                    subq $1, %rax\ntestb %dl, %dl\ncmovneq %rax, %r9\nmovq %r9, -32(%rbp)\n";
        assert_eq!(
            hex(&assemble(find).unwrap()),
            "488b55f8488b4df0488b45e84989c84989c9574889d7fcf2ae0f94c25f4c89c04829c84883e80184d24c0f45c84c894de0"
        );

        // `movq $-1, %rax` だけは GNU as (`48 c7 c0 ff ff ff ff`) と符号化が違う。
        // このアセンブラは 64bit レジスタへの即値を常に `movabs` (`48 b8` + imm64) で出す。
        // どちらも同じ意味で、この違いは `rep` / `test` とは無関係の既存の動作。
        let win_read =
            "testl %eax, %eax\nmovl 40(%rsp), %edx\nmovq $-1, %rax\ncmovneq %rdx, %rax\n";
        assert_eq!(
            hex(&assemble(win_read).unwrap()),
            "85c08b54242848b8ffffffffffffffff480f45c2"
        );
    }

    /// 名前が似ている既存の命令(SSE の `movsd` / `movss`、`movslq`)が変わらないこと
    #[test]
    fn similar_existing_instructions_are_unchanged() {
        check(&[
            ("movsd %xmm0, %xmm1", "f20f10c8"),
            ("movss -4(%rbp), %xmm0", "f30f1045fc"),
            ("movsd -8(%rbp), %xmm2", "f20f1055f8"),
            ("movslq %eax, %rcx", "4863c8"),
            ("cmpltps %xmm1, %xmm0", "0fc2c101"),
        ]);
    }

    #[test]
    fn rep_and_string_instructions_reject_invalid_forms() {
        for source in [
            "rep",
            "rep nop",
            "rep ret",
            "rep mov",
            "rep movsd %xmm0, %xmm1",
            "repnq movsb",
            "movsb %al",
            "stosb (%rdi)",
            "rep stosb %al, (%rdi)",
            "stosd",
            "lodsd",
            "scasd",
            "movs",
            "cld %al",
            "std $1",
        ] {
            assert!(assemble(&format!("{}\n", source)).is_err(), "{}", source);
        }
    }

    #[test]
    fn test_rejects_invalid_operands() {
        for source in [
            "test",
            "test %eax",
            "testl %eax, %ebx, %ecx",
            "testb %eax, %ebx",
            "test %al, %eax",
            "testl $1, %al",
            "testq $0x100000000, %rax",
            "testb $256, %al",
            "test %xmm0, %xmm1",
            "testl $1, foo(%rip)",
            "test $1, $2",
            "testl (%rax), (%rbx)",
        ] {
            assert!(assemble(&format!("{}\n", source)).is_err(), "{}", source);
        }
    }
}
