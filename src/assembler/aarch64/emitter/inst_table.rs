//! 命令の対応表と符号化
//!
//! # 命令を追加するには
//!
//! 1. 下の `dispatch` の `match` に **1行** 足す (ニーモニック -> `emit_xxx`)。
//! 2. 同じファイルの「符号化」に `emit_xxx` を書く。
//!    見本は `emit_fixed` (オペランド無し) と `emit_div` (3レジスタ) 。
//!
//! 固定の1語で済む命令 (`nop` など) は `fixed_word` の表に1行足すだけでよい。
//!
//! # 符号化の道具
//!
//! - オペランドの取り出し: `reg` / `imm` / `symbol` / `expect_len`
//! - 命令語の書き出し: `e.emit_u32(word)`
//! - ラベル参照: `e.add_fixup(symbol, FixupKind::..)` を `emit_u32` の **前** に呼ぶ
//!
//! # 全体の方針
//!
//! レジスタのビット幅 (`%x` / `%w`) から `sf` ビットを決める。
//! 番号 31 は `%sp` と `%xzr` / `%wzr` で意味が変わるので、命令ごとに
//! `Reg::is_sp()` / `Reg::is_zr()` で使えるほうを検査する。

use super::super::parse::{AddrMode, Instruction, MemOffset, MemoryOperand, Operand};
use super::super::reg::Reg;
use super::{Emitter, FixupKind};

type Res = Result<(), String>;

// ============================================================
// ディスパッチ表
// ============================================================

pub(super) fn dispatch(e: &mut Emitter, inst: &Instruction) -> Res {
    let ops = inst.operands.as_slice();
    let m = inst.mnemonic.as_str();

    if let Some(word) = fixed_word(m) {
        return emit_fixed(e, ops, word);
    }

    match m {
        // ---- 分岐 ----
        "b" => emit_branch(e, ops, 0x1400_0000, FixupKind::Jump26),
        "bl" => emit_branch(e, ops, 0x9400_0000, FixupKind::Call26),
        "br" => emit_branch_reg(e, ops, 0xd61f_0000),
        "blr" => emit_branch_reg(e, ops, 0xd63f_0000),
        "ret" => emit_ret(e, ops),
        "cbz" => emit_cbz(e, ops, false),
        "cbnz" => emit_cbz(e, ops, true),
        "tbz" => emit_tbz(e, ops, false),
        "tbnz" => emit_tbz(e, ops, true),
        _ if m.starts_with("b.") => emit_bcond(e, &m[2..], ops),

        // ---- システム ----
        "svc" => emit_imm16(e, ops, 0xd400_0001),
        "brk" => emit_imm16(e, ops, 0xd420_0000),

        // ---- 移動 ----
        "mov" => emit_mov(e, ops),
        "movz" => emit_movwide(e, ops, 0b10),
        "movn" => emit_movwide(e, ops, 0b00),
        "movk" => emit_movwide(e, ops, 0b11),

        // ---- 算術 ----
        "add" | "adds" | "sub" | "subs" => emit_addsub(e, m, ops),
        "cmp" => emit_cmp(e, "subs", ops),
        "cmn" => emit_cmp(e, "adds", ops),
        "neg" => emit_neg(e, "sub", ops),
        "negs" => emit_neg(e, "subs", ops),
        "mul" => emit_madd(e, ops, false, true),
        "mneg" => emit_madd(e, ops, true, true),
        "madd" => emit_madd(e, ops, false, false),
        "msub" => emit_madd(e, ops, true, false),
        "sdiv" => emit_div(e, ops, true),
        "udiv" => emit_div(e, ops, false),
        "smulh" => emit_mulh(e, ops, 0x9b40_7c00),
        "umulh" => emit_mulh(e, ops, 0x9bc0_7c00),

        // ---- 論理 ----
        "and" => emit_logic(e, ops, 0b00, false),
        "orr" => emit_logic(e, ops, 0b01, false),
        "eor" => emit_logic(e, ops, 0b10, false),
        "ands" => emit_logic(e, ops, 0b11, false),
        "bic" => emit_logic(e, ops, 0b00, true),
        "orn" => emit_logic(e, ops, 0b01, true),
        "eon" => emit_logic(e, ops, 0b10, true),
        "bics" => emit_logic(e, ops, 0b11, true),
        "tst" => emit_tst(e, ops),
        "mvn" => emit_mvn(e, ops),

        // ---- シフト / 拡張 ----
        "lsl" => emit_shift(e, ops, 0),
        "lsr" => emit_shift(e, ops, 1),
        "asr" => emit_shift(e, ops, 2),
        "ror" => emit_shift(e, ops, 3),
        "sxtb" => emit_extend(e, ops, true, 7),
        "sxth" => emit_extend(e, ops, true, 15),
        "sxtw" => emit_extend(e, ops, true, 31),
        "uxtb" => emit_extend(e, ops, false, 7),
        "uxth" => emit_extend(e, ops, false, 15),

        // ---- 条件選択 ----
        "csel" => emit_csel(e, ops, 0, 0),
        "csinc" => emit_csel(e, ops, 0, 1),
        "csinv" => emit_csel(e, ops, 1, 0),
        "csneg" => emit_csel(e, ops, 1, 1),
        "cset" => emit_cset(e, ops, 0, 1),
        "csetm" => emit_cset(e, ops, 1, 0),

        // ---- アドレス生成 ----
        "adr" => emit_adr(e, ops, false),
        "adrp" => emit_adr(e, ops, true),

        // ---- ロード / ストア ----
        "ldr" | "str" | "ldrb" | "strb" | "ldrh" | "strh" | "ldrsb" | "ldrsh" | "ldrsw" => {
            emit_ldst(e, m, ops, false)
        }
        "ldur" | "stur" | "ldurb" | "sturb" | "ldurh" | "sturh" | "ldursb" | "ldursh"
        | "ldursw" => emit_ldst(e, &m.replace("ur", "r"), ops, true),
        "ldp" => emit_pair(e, ops, true),
        "stp" => emit_pair(e, ops, false),

        other => Err(format!("unsupported instruction: {}", other)),
    }
}

/// オペランド無しの固定1語の命令
fn fixed_word(m: &str) -> Option<u32> {
    Some(match m {
        "nop" => 0xd503_201f,
        "yield" => 0xd503_203f,
        "wfe" => 0xd503_205f,
        "wfi" => 0xd503_207f,
        "sev" => 0xd503_209f,
        "isb" => 0xd503_3fdf,
        _ => return None,
    })
}

// ============================================================
// オペランドの取り出し
// ============================================================

fn expect_len(ops: &[Operand], min: usize, max: usize) -> Res {
    if ops.len() < min || ops.len() > max {
        if min == max {
            Err(format!("expects {} operand(s), got {}", min, ops.len()))
        } else {
            Err(format!(
                "expects {} to {} operands, got {}",
                min,
                max,
                ops.len()
            ))
        }
    } else {
        Ok(())
    }
}

fn reg(ops: &[Operand], i: usize) -> Result<Reg, String> {
    match ops.get(i) {
        Some(Operand::Register(r)) => Ok(*r),
        Some(other) => Err(format!(
            "operand {} must be a register, got {:?}",
            i + 1,
            other
        )),
        None => Err(format!("missing operand {}", i + 1)),
    }
}

fn imm(ops: &[Operand], i: usize) -> Result<i64, String> {
    match ops.get(i) {
        Some(Operand::Immediate(v)) => Ok(*v),
        Some(other) => Err(format!(
            "operand {} must be an immediate (`$n`), got {:?}",
            i + 1,
            other
        )),
        None => Err(format!("missing operand {}", i + 1)),
    }
}

fn symbol(ops: &[Operand], i: usize) -> Result<&str, String> {
    match ops.get(i) {
        Some(Operand::Symbol(s)) => Ok(s.as_str()),
        Some(other) => Err(format!(
            "operand {} must be a label, got {:?}",
            i + 1,
            other
        )),
        None => Err(format!("missing operand {}", i + 1)),
    }
}

fn same_width(a: Reg, b: Reg) -> Res {
    if a.bits() != b.bits() {
        Err("register size mismatch".to_string())
    } else {
        Ok(())
    }
}

fn sf(r: Reg) -> u32 {
    r.is_64() as u32
}

// ============================================================
// 符号化: 分岐 / システム
// ============================================================

fn emit_fixed(e: &mut Emitter, ops: &[Operand], word: u32) -> Res {
    expect_len(ops, 0, 0)?;
    e.emit_u32(word);
    Ok(())
}

fn emit_branch(e: &mut Emitter, ops: &[Operand], word: u32, kind: FixupKind) -> Res {
    expect_len(ops, 1, 1)?;
    let label = symbol(ops, 0)?;
    e.add_fixup(label, kind);
    e.emit_u32(word);
    Ok(())
}

fn emit_branch_reg(e: &mut Emitter, ops: &[Operand], word: u32) -> Res {
    expect_len(ops, 1, 1)?;
    let rn = reg(ops, 0)?;

    if !rn.is_64() || rn.is_sp() {
        return Err("requires a 64-bit general register".into());
    }

    e.emit_u32(word | (rn.id() << 5));
    Ok(())
}

fn emit_ret(e: &mut Emitter, ops: &[Operand]) -> Res {
    expect_len(ops, 0, 1)?;

    let rn = if ops.is_empty() {
        30
    } else {
        reg(ops, 0)?.id()
    };

    e.emit_u32(0xd65f_0000 | (rn << 5));
    Ok(())
}

const CONDITIONS: &[(&str, u32)] = &[
    ("eq", 0),
    ("ne", 1),
    ("cs", 2),
    ("hs", 2),
    ("cc", 3),
    ("lo", 3),
    ("mi", 4),
    ("pl", 5),
    ("vs", 6),
    ("vc", 7),
    ("hi", 8),
    ("ls", 9),
    ("ge", 10),
    ("lt", 11),
    ("gt", 12),
    ("le", 13),
    ("al", 14),
    ("nv", 15),
];

fn condition(name: &str) -> Result<u32, String> {
    CONDITIONS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
        .ok_or_else(|| format!("unknown condition: {}", name))
}

fn emit_bcond(e: &mut Emitter, cond: &str, ops: &[Operand]) -> Res {
    expect_len(ops, 1, 1)?;
    let cond = condition(cond)?;
    let label = symbol(ops, 0)?;
    e.add_fixup(label, FixupKind::CondBr19);
    e.emit_u32(0x5400_0000 | cond);
    Ok(())
}

fn emit_cbz(e: &mut Emitter, ops: &[Operand], nonzero: bool) -> Res {
    expect_len(ops, 2, 2)?;
    let rt = reg(ops, 0)?;
    let label = symbol(ops, 1)?;

    if rt.is_sp() {
        return Err("%sp cannot be used here".into());
    }

    e.add_fixup(label, FixupKind::CondBr19);
    e.emit_u32((sf(rt) << 31) | 0x3400_0000 | ((nonzero as u32) << 24) | rt.id());
    Ok(())
}

fn emit_tbz(e: &mut Emitter, ops: &[Operand], nonzero: bool) -> Res {
    expect_len(ops, 3, 3)?;
    let rt = reg(ops, 0)?;
    let bit = imm(ops, 1)?;
    let label = symbol(ops, 2)?;

    if rt.is_sp() {
        return Err("%sp cannot be used here".into());
    }

    let max = if rt.is_64() { 63 } else { 31 };

    if !(0..=max).contains(&bit) {
        return Err(format!("bit number must be 0..{}", max));
    }

    let bit = bit as u32;

    e.add_fixup(label, FixupKind::TstBr14);
    e.emit_u32(
        ((bit >> 5) << 31)
            | 0x3600_0000
            | ((nonzero as u32) << 24)
            | ((bit & 0x1f) << 19)
            | rt.id(),
    );
    Ok(())
}

fn emit_imm16(e: &mut Emitter, ops: &[Operand], base: u32) -> Res {
    expect_len(ops, 1, 1)?;
    let v = imm(ops, 0)?;

    if !(0..=0xffff).contains(&v) {
        return Err("immediate must be 0..65535".into());
    }

    e.emit_u32(base | ((v as u32) << 5));
    Ok(())
}

// ============================================================
// 符号化: 移動
// ============================================================

fn emit_mov(e: &mut Emitter, ops: &[Operand]) -> Res {
    expect_len(ops, 2, 2)?;
    let rd = reg(ops, 0)?;

    match &ops[1] {
        Operand::Register(rm) => {
            same_width(rd, *rm)?;

            if rd.is_sp() || rm.is_sp() {
                // mov sp, x0 / mov x0, sp は add rd, rn, #0
                e.emit_u32(addsub_imm_word(rd, *rm, 0, 0, 0, 0));
            } else {
                // orr rd, zr, rm
                let zr = if rd.is_64() { Reg::Xzr } else { Reg::Wzr };
                e.emit_u32(logic_reg_word(rd, zr, *rm, 0b01, false, 0, 0));
            }

            Ok(())
        }

        Operand::Immediate(v) => emit_mov_imm(e, rd, *v),

        other => Err(format!("invalid source operand: {:?}", other)),
    }
}

/// `mov rd, $imm`
///
/// 1命令で表せる形 (movz / movn / ビットマスク即値の orr) があればそれを使い、
/// 無ければ movz + movk の列に展開する。(gas は1命令で表せないとエラーにする)
fn emit_mov_imm(e: &mut Emitter, rd: Reg, v: i64) -> Res {
    let bits = rd.bits();

    let value: u64 = if bits == 32 {
        if v < i32::MIN as i64 || v > u32::MAX as i64 {
            return Err("immediate does not fit in 32 bits".into());
        }

        (v as u32) as u64
    } else {
        v as u64
    };

    let mask: u64 = if bits == 32 { 0xffff_ffff } else { u64::MAX };
    let chunks = (bits / 16) as usize;

    let chunk = |val: u64, i: usize| -> u32 { ((val >> (16 * i)) & 0xffff) as u32 };

    // %sp へは orr (ビットマスク即値) だけ
    if rd.is_sp() {
        let (n, immr, imms) = encode_bitmask(value, bits)
            .ok_or_else(|| "immediate cannot be encoded for %sp".to_string())?;
        let zr = if rd.is_64() { Reg::Xzr } else { Reg::Wzr };
        e.emit_u32(logic_imm_word(rd, zr, 0b01, n, immr, imms));
        return Ok(());
    }

    let nonzero: Vec<usize> = (0..chunks).filter(|&i| chunk(value, i) != 0).collect();

    // movz (0 または 1 チャンクだけが非ゼロ)
    if nonzero.len() <= 1 {
        let hw = nonzero.first().copied().unwrap_or(0);
        e.emit_u32(movwide_word(rd, 0b10, hw as u32, chunk(value, hw)));
        return Ok(());
    }

    // movn (反転すると 1 チャンクだけが非ゼロ)
    let inverted = !value & mask;
    let inv_nonzero: Vec<usize> = (0..chunks).filter(|&i| chunk(inverted, i) != 0).collect();

    if inv_nonzero.len() <= 1 {
        let hw = inv_nonzero.first().copied().unwrap_or(0);
        e.emit_u32(movwide_word(rd, 0b00, hw as u32, chunk(inverted, hw)));
        return Ok(());
    }

    // orr (ビットマスク即値)
    if let Some((n, immr, imms)) = encode_bitmask(value, bits) {
        let zr = if rd.is_64() { Reg::Xzr } else { Reg::Wzr };
        e.emit_u32(logic_imm_word(rd, zr, 0b01, n, immr, imms));
        return Ok(());
    }

    // movz + movk
    let first = nonzero[0];
    e.emit_u32(movwide_word(rd, 0b10, first as u32, chunk(value, first)));

    for &i in &nonzero[1..] {
        e.emit_u32(movwide_word(rd, 0b11, i as u32, chunk(value, i)));
    }

    Ok(())
}

fn movwide_word(rd: Reg, opc: u32, hw: u32, imm16: u32) -> u32 {
    (sf(rd) << 31) | (opc << 29) | (0b100101 << 23) | (hw << 21) | (imm16 << 5) | rd.id()
}

fn emit_movwide(e: &mut Emitter, ops: &[Operand], opc: u32) -> Res {
    expect_len(ops, 2, 3)?;
    let rd = reg(ops, 0)?;
    let v = imm(ops, 1)?;

    if rd.is_sp() || rd.is_zr() {
        return Err("destination must be a general register".into());
    }

    if !(0..=0xffff).contains(&v) {
        return Err("immediate must be 0..65535".into());
    }

    let shift = match ops.get(2) {
        None => 0,
        Some(Operand::Shift(kind, Some(amount))) if kind == "lsl" => *amount,
        Some(other) => return Err(format!("expected `lsl $n`, got {:?}", other)),
    };

    let max = if rd.is_64() { 48 } else { 16 };

    if shift < 0 || shift > max || shift % 16 != 0 {
        return Err(format!(
            "shift must be 0, 16{}",
            if rd.is_64() { ", 32 or 48" } else { "" }
        ));
    }

    e.emit_u32(movwide_word(rd, opc, (shift / 16) as u32, v as u32));
    Ok(())
}

// ============================================================
// 符号化: 算術
// ============================================================

fn addsub_imm_word(rd: Reg, rn: Reg, op: u32, s: u32, sh: u32, imm12: u32) -> u32 {
    (sf(rd) << 31)
        | (op << 30)
        | (s << 29)
        | (0b100010 << 23)
        | (sh << 22)
        | (imm12 << 10)
        | (rn.id() << 5)
        | rd.id()
}

fn extend_option(name: &str, is64: bool) -> Result<u32, String> {
    Ok(match name {
        "uxtb" => 0,
        "uxth" => 1,
        "uxtw" => 2,
        "uxtx" => 3,
        "sxtb" => 4,
        "sxth" => 5,
        "sxtw" => 6,
        "sxtx" => 7,
        // lsl は 64bit なら uxtx、32bit なら uxtw の別名
        "lsl" => {
            if is64 {
                3
            } else {
                2
            }
        }
        other => return Err(format!("invalid extend: {}", other)),
    })
}

fn shift_kind(name: &str, allow_ror: bool) -> Result<u32, String> {
    match name {
        "lsl" => Ok(0),
        "lsr" => Ok(1),
        "asr" => Ok(2),
        "ror" if allow_ror => Ok(3),
        other => Err(format!("invalid shift: {}", other)),
    }
}

fn emit_addsub(e: &mut Emitter, name: &str, ops: &[Operand]) -> Res {
    expect_len(ops, 3, 4)?;

    let mut op = name.starts_with("sub") as u32;
    let s = name.ends_with('s') as u32;

    let rd = reg(ops, 0)?;
    let rn = reg(ops, 1)?;

    if s == 1 && rd.is_sp() {
        return Err("%sp cannot be the destination of a flag-setting instruction".into());
    }

    match &ops[2] {
        Operand::Immediate(v) => {
            same_width(rd, rn)?;

            if rn.is_zr() {
                return Err("%xzr/%wzr cannot be the source here".into());
            }

            if rd.is_zr() && s == 0 {
                return Err("%xzr/%wzr cannot be the destination here".into());
            }

            let mut v = *v;

            // 負の即値は add <-> sub を入れ替える (gas と同じ)
            if v < 0 {
                v = v.checked_neg().ok_or("immediate out of range")?;
                op ^= 1;
            }

            let (sh, imm12) = match ops.get(3) {
                None => split_addsub_imm(v)?,
                Some(Operand::Shift(kind, Some(amount))) if kind == "lsl" => match amount {
                    0 if v <= 0xfff => (0, v as u32),
                    12 if v <= 0xfff => (1, v as u32),
                    _ => return Err("immediate or shift out of range".into()),
                },
                Some(other) => return Err(format!("expected `lsl $0|$12`, got {:?}", other)),
            };

            e.emit_u32(addsub_imm_word(rd, rn, op, s, sh, imm12));
            Ok(())
        }

        Operand::Lo12(sym) => {
            same_width(rd, rn)?;

            if op != 0 || s != 0 || ops.len() != 3 {
                return Err(":lo12: is only valid with `add`".into());
            }

            if !rd.is_64() {
                return Err(":lo12: requires 64-bit registers".into());
            }

            e.add_fixup(sym, FixupKind::AddLo12);
            e.emit_u32(addsub_imm_word(rd, rn, 0, 0, 0, 0));
            Ok(())
        }

        Operand::Register(rm) => {
            let ext = match ops.get(3) {
                None => None,
                Some(Operand::Shift(kind, amount)) => Some((kind.as_str(), *amount)),
                Some(other) => return Err(format!("expected a shift or extend, got {:?}", other)),
            };

            let is_extend =
                matches!(ext, Some((k, _)) if k != "lsl" && k != "lsr" && k != "asr" && k != "ror");

            // rd / rn が %sp のとき、または拡張指定があるときは拡張レジスタ形式
            if rd.is_sp() || rn.is_sp() || is_extend {
                if rm.is_sp() {
                    return Err("%sp cannot be the third operand".into());
                }

                if rn.bits() != rd.bits() {
                    return Err("register size mismatch".into());
                }

                let (kind, amount) = ext.unwrap_or(("lsl", None));
                let option = extend_option(kind, rd.is_64())?;
                let amount = amount.unwrap_or(0);

                if !(0..=4).contains(&amount) {
                    return Err("extend shift amount must be 0..4".into());
                }

                // Wm を使えるのは uxtb/uxth/uxtw/sxtb/sxth/sxtw (option の最下位2bit が 3 でない)
                let wide_rm = option & 3 == 3;

                if rm.is_64() != wide_rm && !(rd.is_64() == false && !rm.is_64()) {
                    return Err("register size mismatch for the extend".into());
                }

                e.emit_u32(
                    (sf(rd) << 31)
                        | (op << 30)
                        | (s << 29)
                        | (0b01011 << 24)
                        | (1 << 21)
                        | (rm.id() << 16)
                        | (option << 13)
                        | ((amount as u32) << 10)
                        | (rn.id() << 5)
                        | rd.id(),
                );
                return Ok(());
            }

            same_width(rd, rn)?;
            same_width(rd, *rm)?;

            let (kind, amount) = ext.unwrap_or(("lsl", None));
            let shift = shift_kind(kind, false)?;
            let amount = amount.unwrap_or(0);

            if amount < 0 || amount >= rd.bits() as i64 {
                return Err("shift amount out of range".into());
            }

            e.emit_u32(
                (sf(rd) << 31)
                    | (op << 30)
                    | (s << 29)
                    | (0b01011 << 24)
                    | (shift << 22)
                    | (rm.id() << 16)
                    | ((amount as u32) << 10)
                    | (rn.id() << 5)
                    | rd.id(),
            );
            Ok(())
        }

        other => Err(format!("invalid third operand: {:?}", other)),
    }
}

/// 12bit 即値、または 12bit 即値 << 12 に分解する
fn split_addsub_imm(v: i64) -> Result<(u32, u32), String> {
    if v <= 0xfff {
        Ok((0, v as u32))
    } else if v & 0xfff == 0 && (v >> 12) <= 0xfff {
        Ok((1, (v >> 12) as u32))
    } else {
        Err("immediate out of range (0..4095, optionally << 12)".into())
    }
}

/// `cmp rn, x` = `subs zr, rn, x` / `cmn rn, x` = `adds zr, rn, x`
fn emit_cmp(e: &mut Emitter, name: &str, ops: &[Operand]) -> Res {
    expect_len(ops, 2, 3)?;
    let rn = reg(ops, 0)?;
    let zr = if rn.is_64() { Reg::Xzr } else { Reg::Wzr };

    let mut full = vec![Operand::Register(zr)];
    full.extend_from_slice(ops);

    emit_addsub(e, name, &full)
}

/// `neg rd, rm` = `sub rd, zr, rm`
fn emit_neg(e: &mut Emitter, name: &str, ops: &[Operand]) -> Res {
    expect_len(ops, 2, 3)?;
    let rd = reg(ops, 0)?;
    let zr = if rd.is_64() { Reg::Xzr } else { Reg::Wzr };

    let mut full = vec![ops[0].clone(), Operand::Register(zr)];
    full.extend_from_slice(&ops[1..]);

    emit_addsub(e, name, &full)
}

fn emit_madd(e: &mut Emitter, ops: &[Operand], sub: bool, short: bool) -> Res {
    let (rd, rn, rm, ra);

    if short {
        // mul / mneg rd, rn, rm
        expect_len(ops, 3, 3)?;
        rd = reg(ops, 0)?;
        rn = reg(ops, 1)?;
        rm = reg(ops, 2)?;
        ra = if rd.is_64() { Reg::Xzr } else { Reg::Wzr };
    } else {
        // madd / msub rd, rn, rm, ra
        expect_len(ops, 4, 4)?;
        rd = reg(ops, 0)?;
        rn = reg(ops, 1)?;
        rm = reg(ops, 2)?;
        ra = reg(ops, 3)?;
    }

    for r in [rd, rn, rm, ra] {
        same_width(rd, r)?;

        if r.is_sp() {
            return Err("%sp cannot be used here".into());
        }
    }

    e.emit_u32(
        (sf(rd) << 31)
            | (0b0011011000 << 21)
            | (rm.id() << 16)
            | ((sub as u32) << 15)
            | (ra.id() << 10)
            | (rn.id() << 5)
            | rd.id(),
    );
    Ok(())
}

fn emit_div(e: &mut Emitter, ops: &[Operand], signed: bool) -> Res {
    expect_len(ops, 3, 3)?;
    let rd = reg(ops, 0)?;
    let rn = reg(ops, 1)?;
    let rm = reg(ops, 2)?;

    for r in [rd, rn, rm] {
        same_width(rd, r)?;

        if r.is_sp() {
            return Err("%sp cannot be used here".into());
        }
    }

    e.emit_u32(
        (sf(rd) << 31)
            | (0b0011010110 << 21)
            | (rm.id() << 16)
            | ((0b000010 | signed as u32) << 10)
            | (rn.id() << 5)
            | rd.id(),
    );
    Ok(())
}

fn emit_mulh(e: &mut Emitter, ops: &[Operand], base: u32) -> Res {
    expect_len(ops, 3, 3)?;
    let rd = reg(ops, 0)?;
    let rn = reg(ops, 1)?;
    let rm = reg(ops, 2)?;

    for r in [rd, rn, rm] {
        if !r.is_64() || r.is_sp() {
            return Err("requires 64-bit general registers".into());
        }
    }

    e.emit_u32(base | (rm.id() << 16) | (rn.id() << 5) | rd.id());
    Ok(())
}

// ============================================================
// 符号化: 論理
// ============================================================

fn logic_reg_word(rd: Reg, rn: Reg, rm: Reg, opc: u32, n: bool, shift: u32, amount: u32) -> u32 {
    (sf(rd) << 31)
        | (opc << 29)
        | (0b01010 << 24)
        | (shift << 22)
        | ((n as u32) << 21)
        | (rm.id() << 16)
        | (amount << 10)
        | (rn.id() << 5)
        | rd.id()
}

fn logic_imm_word(rd: Reg, rn: Reg, opc: u32, n: u32, immr: u32, imms: u32) -> u32 {
    (sf(rd) << 31)
        | (opc << 29)
        | (0b100100 << 23)
        | (n << 22)
        | (immr << 16)
        | (imms << 10)
        | (rn.id() << 5)
        | rd.id()
}

fn emit_logic(e: &mut Emitter, ops: &[Operand], opc: u32, invert: bool) -> Res {
    expect_len(ops, 3, 4)?;
    let rd = reg(ops, 0)?;
    let rn = reg(ops, 1)?;

    same_width(rd, rn)?;

    if rn.is_sp() {
        return Err("%sp cannot be the source here".into());
    }

    match &ops[2] {
        Operand::Immediate(v) if !invert => {
            let flags = opc == 0b11;

            if flags && rd.is_sp() {
                return Err("%sp cannot be the destination of a flag-setting instruction".into());
            }

            let bits = rd.bits();

            let value: u64 = if bits == 32 {
                if *v < i32::MIN as i64 || *v > u32::MAX as i64 {
                    return Err("immediate does not fit in 32 bits".into());
                }
                (*v as u32) as u64
            } else {
                *v as u64
            };

            let (n, immr, imms) = encode_bitmask(value, bits)
                .ok_or_else(|| format!("immediate {:#x} cannot be encoded as a bitmask", v))?;

            e.emit_u32(logic_imm_word(rd, rn, opc, n, immr, imms));
            Ok(())
        }

        Operand::Register(rm) => {
            same_width(rd, *rm)?;

            if rd.is_sp() || rm.is_sp() {
                return Err("%sp cannot be used here".into());
            }

            let (shift, amount) = shift_operand(ops.get(3), rd.bits())?;

            e.emit_u32(logic_reg_word(rd, rn, *rm, opc, invert, shift, amount));
            Ok(())
        }

        other => Err(format!("invalid third operand: {:?}", other)),
    }
}

/// 省略可能な `lsl $n` などを (種類, 量) にする (ror 可)
fn shift_operand(op: Option<&Operand>, bits: u32) -> Result<(u32, u32), String> {
    match op {
        None => Ok((0, 0)),
        Some(Operand::Shift(kind, amount)) => {
            let shift = shift_kind(kind, true)?;
            let amount = amount.unwrap_or(0);

            if amount < 0 || amount >= bits as i64 {
                return Err("shift amount out of range".into());
            }

            Ok((shift, amount as u32))
        }
        Some(other) => Err(format!("expected a shift, got {:?}", other)),
    }
}

fn emit_tst(e: &mut Emitter, ops: &[Operand]) -> Res {
    expect_len(ops, 2, 3)?;
    let rn = reg(ops, 0)?;
    let zr = if rn.is_64() { Reg::Xzr } else { Reg::Wzr };

    let mut full = vec![Operand::Register(zr)];
    full.extend_from_slice(ops);

    emit_logic(e, &full, 0b11, false)
}

fn emit_mvn(e: &mut Emitter, ops: &[Operand]) -> Res {
    expect_len(ops, 2, 3)?;
    let rd = reg(ops, 0)?;
    let zr = if rd.is_64() { Reg::Xzr } else { Reg::Wzr };

    let mut full = vec![ops[0].clone(), Operand::Register(zr)];
    full.extend_from_slice(&ops[1..]);

    emit_logic(e, &full, 0b01, true)
}

/// 論理命令のビットマスク即値の符号化 (N, immr, imms)。
/// 全ビット 0 / 全ビット 1 や、繰り返しパターンで表せない値は `None`。
fn encode_bitmask(value: u64, bits: u32) -> Option<(u32, u32, u32)> {
    let mut imm = value;

    if bits == 32 {
        imm &= 0xffff_ffff;
        imm |= imm << 32;
    }

    if imm == 0 || imm == u64::MAX {
        return None;
    }

    let is_mask = |x: u64| x != 0 && (x.wrapping_add(1) & x) == 0;
    let is_shifted_mask = |x: u64| x != 0 && is_mask((x.wrapping_sub(1)) | x);

    // 繰り返しの最小単位 (2, 4, 8, 16, 32, 64) を探す
    let mut size: u32 = 64;

    loop {
        size /= 2;
        let m = (1u64 << size) - 1;

        if (imm & m) != ((imm >> size) & m) {
            size *= 2;
            break;
        }

        if size <= 2 {
            break;
        }
    }

    let mask = u64::MAX >> (64 - size);
    let mut pattern = imm & mask;

    let (i, cto) = if is_shifted_mask(pattern) {
        let i = pattern.trailing_zeros();
        (i, (pattern >> i).trailing_ones())
    } else {
        pattern |= !mask;

        if !is_shifted_mask(!pattern) {
            return None;
        }

        let clo = pattern.leading_ones();
        (64 - clo, clo + pattern.trailing_ones() - (64 - size))
    };

    let immr = (size - i) & (size - 1);

    let mut nimms: u64 = (!(size as u64 - 1)) << 1;
    nimms |= (cto - 1) as u64;

    let n = (((nimms >> 6) & 1) ^ 1) as u32;

    Some((n, immr, (nimms & 0x3f) as u32))
}

// ============================================================
// 符号化: シフト / 拡張
// ============================================================

fn bfm_word(rd: Reg, opc: u32, immr: u32, imms: u32, rn: Reg) -> u32 {
    (sf(rd) << 31)
        | (opc << 29)
        | (0b100110 << 23)
        | (sf(rd) << 22)
        | (immr << 16)
        | (imms << 10)
        | (rn.id() << 5)
        | rd.id()
}

/// `lsl` / `lsr` / `asr` / `ror` (kind: 0..3)
fn emit_shift(e: &mut Emitter, ops: &[Operand], kind: u32) -> Res {
    expect_len(ops, 3, 3)?;
    let rd = reg(ops, 0)?;
    let rn = reg(ops, 1)?;

    same_width(rd, rn)?;

    if rd.is_sp() || rn.is_sp() {
        return Err("%sp cannot be used here".into());
    }

    let bits = rd.bits();

    match &ops[2] {
        Operand::Register(rm) => {
            same_width(rd, *rm)?;

            // lslv / lsrv / asrv / rorv
            e.emit_u32(
                (sf(rd) << 31)
                    | (0b0011010110 << 21)
                    | (rm.id() << 16)
                    | (0b0010 << 12)
                    | (kind << 10)
                    | (rn.id() << 5)
                    | rd.id(),
            );
            Ok(())
        }

        Operand::Immediate(v) => {
            if *v < 0 || *v >= bits as i64 {
                return Err(format!("shift amount must be 0..{}", bits - 1));
            }

            let v = *v as u32;

            let word = match kind {
                // lsl: ubfm rd, rn, #(-v mod bits), #(bits-1-v)
                0 => bfm_word(rd, 0b10, (bits - v) % bits, bits - 1 - v, rn),
                // lsr: ubfm rd, rn, #v, #(bits-1)
                1 => bfm_word(rd, 0b10, v, bits - 1, rn),
                // asr: sbfm rd, rn, #v, #(bits-1)
                2 => bfm_word(rd, 0b00, v, bits - 1, rn),
                // ror: extr rd, rn, rn, #v
                _ => {
                    (sf(rd) << 31)
                        | (0b00100111 << 23)
                        | (sf(rd) << 22)
                        | (rn.id() << 16)
                        | (v << 10)
                        | (rn.id() << 5)
                        | rd.id()
                }
            };

            e.emit_u32(word);
            Ok(())
        }

        other => Err(format!("invalid shift amount: {:?}", other)),
    }
}

/// `sxtb` / `sxth` / `sxtw` / `uxtb` / `uxth` (`imms` は 7 / 15 / 31)
fn emit_extend(e: &mut Emitter, ops: &[Operand], signed: bool, imms: u32) -> Res {
    expect_len(ops, 2, 2)?;
    let rd = reg(ops, 0)?;
    let rn = reg(ops, 1)?;

    if rd.is_sp() || rn.is_sp() {
        return Err("%sp cannot be used here".into());
    }

    // 入力は常にW、出力は sxt* だけ X も可 (sxtw は X のみ)
    if rn.is_64() {
        return Err("source must be a 32-bit register (%wN)".into());
    }

    if !signed && rd.is_64() {
        return Err("destination must be a 32-bit register (%wN)".into());
    }

    if imms == 31 && !rd.is_64() {
        return Err("sxtw requires a 64-bit destination".into());
    }

    let opc = if signed { 0b00 } else { 0b10 };

    e.emit_u32(bfm_word(rd, opc, 0, imms, rn));
    Ok(())
}

// ============================================================
// 符号化: 条件選択
// ============================================================

fn emit_csel(e: &mut Emitter, ops: &[Operand], op: u32, o2: u32) -> Res {
    expect_len(ops, 4, 4)?;
    let rd = reg(ops, 0)?;
    let rn = reg(ops, 1)?;
    let rm = reg(ops, 2)?;
    let cond = condition(symbol(ops, 3)?)?;

    for r in [rd, rn, rm] {
        same_width(rd, r)?;

        if r.is_sp() {
            return Err("%sp cannot be used here".into());
        }
    }

    e.emit_u32(csel_word(rd, rn, rm, op, o2, cond));
    Ok(())
}

fn csel_word(rd: Reg, rn: Reg, rm: Reg, op: u32, o2: u32, cond: u32) -> u32 {
    (sf(rd) << 31)
        | (op << 30)
        | (0b011010100 << 21)
        | (rm.id() << 16)
        | (cond << 12)
        | (o2 << 10)
        | (rn.id() << 5)
        | rd.id()
}

/// `cset rd, cond` = `csinc rd, zr, zr, !cond`、`csetm` = `csinv rd, zr, zr, !cond`
fn emit_cset(e: &mut Emitter, ops: &[Operand], op: u32, o2: u32) -> Res {
    expect_len(ops, 2, 2)?;
    let rd = reg(ops, 0)?;
    let cond = condition(symbol(ops, 1)?)?;

    if rd.is_sp() {
        return Err("%sp cannot be used here".into());
    }

    if cond >= 14 {
        return Err("`al` / `nv` cannot be used here".into());
    }

    let zr = if rd.is_64() { Reg::Xzr } else { Reg::Wzr };

    e.emit_u32(csel_word(rd, zr, zr, op, o2, cond ^ 1));
    Ok(())
}

// ============================================================
// 符号化: アドレス生成
// ============================================================

fn emit_adr(e: &mut Emitter, ops: &[Operand], page: bool) -> Res {
    expect_len(ops, 2, 2)?;
    let rd = reg(ops, 0)?;
    let label = symbol(ops, 1)?;

    if !rd.is_64() || rd.is_sp() {
        return Err("destination must be a 64-bit general register".into());
    }

    let (kind, word) = if page {
        (FixupKind::AdrpPage21, 0x9000_0000)
    } else {
        (FixupKind::AdrLo21, 0x1000_0000)
    };

    e.add_fixup(label, kind);
    e.emit_u32(word | rd.id());
    Ok(())
}

// ============================================================
// 符号化: ロード / ストア
// ============================================================

/// (size, opc) : `ldr`/`str` 系の符号化
fn ldst_spec(name: &str, rt: Reg) -> Result<(u32, u32), String> {
    let load = name.starts_with("ldr");

    if rt.is_sp() {
        return Err("%sp cannot be used as a transfer register".into());
    }

    match name {
        "ldr" | "str" => Ok((if rt.is_64() { 3 } else { 2 }, load as u32)),
        "ldrb" | "strb" | "ldrh" | "strh" => {
            if rt.is_64() {
                return Err("requires a 32-bit register (%wN)".into());
            }

            Ok((if name.ends_with('b') { 0 } else { 1 }, load as u32))
        }
        "ldrsb" | "ldrsh" => {
            let size = if name == "ldrsb" { 0 } else { 1 };
            Ok((size, if rt.is_64() { 2 } else { 3 }))
        }
        "ldrsw" => {
            if !rt.is_64() {
                return Err("requires a 64-bit register (%xN)".into());
            }

            Ok((2, 2))
        }
        other => Err(format!("unsupported load/store: {}", other)),
    }
}

fn emit_ldst(e: &mut Emitter, name: &str, ops: &[Operand], force_unscaled: bool) -> Res {
    expect_len(ops, 2, 2)?;
    let rt = reg(ops, 0)?;
    let (size, opc) = ldst_spec(name, rt)?;

    let mem = match &ops[1] {
        Operand::Memory(m) => m,

        // ldr rt, label (リテラルロード)
        Operand::Symbol(label) if name == "ldr" => {
            e.add_fixup(label, FixupKind::LdrLit19);
            e.emit_u32(0x1800_0000 | ((rt.is_64() as u32) << 30) | rt.id());
            return Ok(());
        }

        other => {
            return Err(format!(
                "operand 2 must be a memory operand, got {:?}",
                other
            ));
        }
    };

    let rn = mem.base;

    if !rn.is_64() || rn.is_zr() {
        return Err("base must be a 64-bit register or %sp".into());
    }

    let head = (size << 30) | (opc << 22) | (rn.id() << 5) | rt.id();

    match (&mem.offset, mem.mode) {
        (MemOffset::None, AddrMode::Offset) => emit_ldst_imm(e, head, size, 0, force_unscaled),
        (MemOffset::Imm(i), AddrMode::Offset) => emit_ldst_imm(e, head, size, *i, force_unscaled),

        (MemOffset::Imm(i), AddrMode::PreIndex) => {
            check_imm9(*i)?;
            e.emit_u32(0x3800_0000 | head | (((*i as u32) & 0x1ff) << 12) | (0b11 << 10));
            Ok(())
        }

        (MemOffset::Imm(i), AddrMode::PostIndex) => {
            check_imm9(*i)?;
            e.emit_u32(0x3800_0000 | head | (((*i as u32) & 0x1ff) << 12) | (0b01 << 10));
            Ok(())
        }

        (MemOffset::Lo12(sym), AddrMode::Offset) => {
            e.add_fixup(sym, FixupKind::LdstLo12(size as u8));
            e.emit_u32(0x3900_0000 | head);
            Ok(())
        }

        (MemOffset::Reg(rm, ext), AddrMode::Offset) => {
            let (kind, amount) = match ext {
                None => ("lsl", None),
                Some((k, a)) => (k.as_str(), *a),
            };

            let option = match kind {
                "lsl" | "uxtx" => 3,
                "uxtw" => 2,
                "sxtw" => 6,
                "sxtx" => 7,
                other => return Err(format!("invalid extend in memory operand: {}", other)),
            };

            // 32bit の index は uxtw / sxtw、64bit の index は lsl / uxtx / sxtx
            if (option & 3 == 2) == rm.is_64() {
                return Err("index register size does not match the extend".into());
            }

            if rm.is_sp() {
                return Err("%sp cannot be used as an index".into());
            }

            let s = match amount {
                None | Some(0) => 0,
                Some(a) if a == size as i64 => 1,
                Some(_) => return Err(format!("shift amount must be 0 or {}", size)),
            };

            e.emit_u32(0x3820_0800 | head | (rm.id() << 16) | (option << 13) | (s << 12));
            Ok(())
        }

        _ => Err("unsupported addressing mode".into()),
    }
}

fn check_imm9(i: i64) -> Res {
    if (-256..=255).contains(&i) {
        Ok(())
    } else {
        Err("offset must be -256..255".into())
    }
}

/// オフセット付き (インデックス更新なし) の ldr/str。
/// 4096 * サイズ 未満の正のオフセットで境界が合っていれば `unsigned offset` 形式、
/// そうでなければ -256..255 の `unscaled` 形式 (ldur/stur) にする (gas と同じ)。
fn emit_ldst_imm(e: &mut Emitter, head: u32, size: u32, offset: i64, force_unscaled: bool) -> Res {
    let unit = 1i64 << size;

    if !force_unscaled && offset >= 0 && offset % unit == 0 && offset / unit <= 0xfff {
        e.emit_u32(0x3900_0000 | head | (((offset / unit) as u32) << 10));
        return Ok(());
    }

    if (-256..=255).contains(&offset) {
        e.emit_u32(0x3800_0000 | head | (((offset as u32) & 0x1ff) << 12));
        return Ok(());
    }

    Err(format!(
        "offset {} is out of range (unsigned: 0..{} step {}, unscaled: -256..255)",
        offset,
        0xfff * unit,
        unit
    ))
}

fn emit_pair(e: &mut Emitter, ops: &[Operand], load: bool) -> Res {
    expect_len(ops, 3, 3)?;
    let rt = reg(ops, 0)?;
    let rt2 = reg(ops, 1)?;

    same_width(rt, rt2)?;

    if rt.is_sp() || rt2.is_sp() {
        return Err("%sp cannot be used as a transfer register".into());
    }

    let mem: &MemoryOperand = match &ops[2] {
        Operand::Memory(m) => m,
        other => {
            return Err(format!(
                "operand 3 must be a memory operand, got {:?}",
                other
            ));
        }
    };

    let rn = mem.base;

    if !rn.is_64() || rn.is_zr() {
        return Err("base must be a 64-bit register or %sp".into());
    }

    let offset = match &mem.offset {
        MemOffset::None => 0,
        MemOffset::Imm(i) => *i,
        _ => return Err("only an immediate offset is allowed".into()),
    };

    let scale = if rt.is_64() { 8 } else { 4 };

    if offset % scale != 0 || !(-64..=63).contains(&(offset / scale)) {
        return Err(format!(
            "offset must be a multiple of {} in {}..{}",
            scale,
            -64 * scale,
            63 * scale
        ));
    }

    let mode = match mem.mode {
        AddrMode::Offset => 0b010,
        AddrMode::PreIndex => 0b011,
        AddrMode::PostIndex => 0b001,
    };

    e.emit_u32(
        ((rt.is_64() as u32) << 31)
            | (0b101 << 27)
            | (mode << 23)
            | ((load as u32) << 22)
            | ((((offset / scale) as u32) & 0x7f) << 15)
            | (rt2.id() << 10)
            | (rn.id() << 5)
            | rt.id(),
    );
    Ok(())
}

// ============================================================
// テスト
//
// 期待値は GNU as (aarch64-linux-gnu-as) の出力から作っている。
// `mov` で1命令に収まらない値だけは gas がエラーにするので、
// 同じ値になる movz / movk の列と比べている。
// ============================================================

#[cfg(test)]
mod tests {
    use super::super::super::assemble_aarch64;

    fn words(src: &str) -> Vec<u32> {
        let (text, _) = assemble_aarch64(src).unwrap_or_else(|e| panic!("{}: {}", src, e));

        text.chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    const CASES: &[(&str, &[u32])] = &[
        ("nop", &[0xd503201f]),
        ("mov %x0, %x1", &[0xaa0103e0]),
        ("mov %w0, %w1", &[0x2a0103e0]),
        ("mov %x0, $0", &[0xd2800000]),
        ("mov %x0, $1", &[0xd2800020]),
        ("mov %x0, $65535", &[0xd29fffe0]),
        ("mov %x0, $-1", &[0x92800000]),
        ("mov %w0, $-1", &[0x12800000]),
        ("mov %x1, $0x10000", &[0xd2a00021]),
        ("mov %x2, $-2", &[0x92800022]),
        ("mov %x3, $0xff00ff00ff00ff00", &[0xb2089fe3]),
        ("mov %x3, $0x5555555555555555", &[0xb200f3e3]),
        ("mov %w4, $0xaaaaaaaa", &[0x3201f3e4]),
        ("mov %sp, %x1", &[0x9100003f]),
        ("mov %x1, %sp", &[0x910003e1]),
        ("mov %x5, $0x12345678", &[0xd28acf05, 0xf2a24685]),
        (
            "mov %x5, $0x123456789abcdef0",
            &[0xd29bde05, 0xf2b35785, 0xf2cacf05, 0xf2e24685],
        ),
        ("movz %x0, $1, lsl $16", &[0xd2a00020]),
        ("movk %x0, $0xbeef, lsl $48", &[0xf2f7dde0]),
        ("movn %w1, $5", &[0x128000a1]),
        ("add %x0, %x1, $1", &[0x91000420]),
        ("add %x0, %x1, $4095", &[0x913ffc20]),
        ("add %x0, %x1, $0x1000", &[0x91400420]),
        ("add %x0, %x1, $1, lsl $12", &[0x91400420]),
        ("add %w0, %w1, $7", &[0x11001c20]),
        ("add %x0, %x1, $-8", &[0xd1002020]),
        ("sub %sp, %sp, $16", &[0xd10043ff]),
        ("sub %x0, %x1, %x2", &[0xcb020020]),
        ("add %x0, %x1, %x2, lsl $3", &[0x8b020c20]),
        ("add %w0, %w1, %w2, lsr $5", &[0x0b421420]),
        ("sub %x0, %x1, %x2, asr $1", &[0xcb820420]),
        ("adds %x0, %x1, %x2", &[0xab020020]),
        ("subs %w0, %w1, $3", &[0x71000c20]),
        ("add %sp, %sp, %x1", &[0x8b2163ff]),
        ("add %x0, %x1, %w2, sxtw", &[0x8b22c020]),
        ("add %x0, %x1, %w2, uxtw $2", &[0x8b224820]),
        ("add %x0, %sp, $8", &[0x910023e0]),
        ("cmp %x0, $5", &[0xf100141f]),
        ("cmp %w0, %w1", &[0x6b01001f]),
        ("cmp %x0, %x1, lsl $2", &[0xeb01081f]),
        ("cmn %x0, $1", &[0xb100041f]),
        ("cmp %sp, $16", &[0xf10043ff]),
        ("neg %x0, %x1", &[0xcb0103e0]),
        ("neg %w0, %w1", &[0x4b0103e0]),
        ("negs %x0, %x1", &[0xeb0103e0]),
        ("mul %x0, %x1, %x2", &[0x9b027c20]),
        ("mul %w0, %w1, %w2", &[0x1b027c20]),
        ("mneg %x0, %x1, %x2", &[0x9b02fc20]),
        ("madd %x0, %x1, %x2, %x3", &[0x9b020c20]),
        ("msub %x0, %x1, %x2, %x3", &[0x9b028c20]),
        ("sdiv %x0, %x1, %x2", &[0x9ac20c20]),
        ("udiv %w0, %w1, %w2", &[0x1ac20820]),
        ("smulh %x0, %x1, %x2", &[0x9b427c20]),
        ("umulh %x0, %x1, %x2", &[0x9bc27c20]),
        ("and %x0, %x1, %x2", &[0x8a020020]),
        ("orr %x0, %x1, %x2, lsl $4", &[0xaa021020]),
        ("eor %w0, %w1, %w2", &[0x4a020020]),
        ("ands %x0, %x1, %x2", &[0xea020020]),
        ("bic %x0, %x1, %x2", &[0x8a220020]),
        ("orn %x0, %x1, %x2", &[0xaa220020]),
        ("eon %x0, %x1, %x2", &[0xca220020]),
        ("bics %x0, %x1, %x2", &[0xea220020]),
        ("and %x0, %x1, $0xff", &[0x92401c20]),
        ("orr %x0, %x1, $0xf0f0f0f0f0f0f0f0", &[0xb204cc20]),
        ("eor %w0, %w1, $0x55555555", &[0x5200f020]),
        ("ands %x0, %x1, $1", &[0xf2400020]),
        ("and %w0, %w1, $0x7fffffff", &[0x12007820]),
        ("orr %sp, %x1, $0xff", &[0xb2401c3f]),
        ("tst %x0, $1", &[0xf240001f]),
        ("tst %x0, %x1", &[0xea01001f]),
        ("tst %w0, $0xff", &[0x72001c1f]),
        ("mvn %x0, %x1", &[0xaa2103e0]),
        ("mvn %w0, %w1, lsl $2", &[0x2a210be0]),
        ("lsl %x0, %x1, $3", &[0xd37df020]),
        ("lsl %w0, %w1, $3", &[0x531d7020]),
        ("lsr %x0, %x1, $63", &[0xd37ffc20]),
        ("lsr %w0, %w1, $1", &[0x53017c20]),
        ("asr %x0, %x1, $7", &[0x9347fc20]),
        ("asr %w0, %w1, $31", &[0x131f7c20]),
        ("ror %x0, %x1, $8", &[0x93c12020]),
        ("lsl %x0, %x1, %x2", &[0x9ac22020]),
        ("lsr %w0, %w1, %w2", &[0x1ac22420]),
        ("asr %x0, %x1, %x2", &[0x9ac22820]),
        ("ror %w0, %w1, %w2", &[0x1ac22c20]),
        ("lsl %x0, %x1, $0", &[0xd340fc20]),
        ("sxtb %w0, %w1", &[0x13001c20]),
        ("sxtb %x0, %w1", &[0x93401c20]),
        ("sxth %x0, %w1", &[0x93403c20]),
        ("sxtw %x0, %w1", &[0x93407c20]),
        ("uxtb %w0, %w1", &[0x53001c20]),
        ("uxth %w0, %w1", &[0x53003c20]),
        ("csel %x0, %x1, %x2, eq", &[0x9a820020]),
        ("csinc %w0, %w1, %w2, ne", &[0x1a821420]),
        ("csinv %x0, %x1, %x2, lt", &[0xda82b020]),
        ("csneg %x0, %x1, %x2, hs", &[0xda822420]),
        ("cset %x0, eq", &[0x9a9f17e0]),
        ("cset %w0, ge", &[0x1a9fb7e0]),
        ("csetm %x0, ne", &[0xda9f03e0]),
        ("ret", &[0xd65f03c0]),
        ("ret %x1", &[0xd65f0020]),
        ("br %x16", &[0xd61f0200]),
        ("blr %x8", &[0xd63f0100]),
        ("svc $0", &[0xd4000001]),
        ("brk $1000", &[0xd4207d00]),
        ("yield", &[0xd503203f]),
        ("wfi", &[0xd503207f]),
        ("isb", &[0xd5033fdf]),
        ("ldr %x0, [%x1]", &[0xf9400020]),
        ("ldr %x0, [%x1, $8]", &[0xf9400420]),
        ("ldr %w0, [%x1, $4]", &[0xb9400420]),
        ("ldr %x0, [%x1, $32760]", &[0xf97ffc20]),
        ("ldr %x0, [%x1, $-8]", &[0xf85f8020]),
        ("ldr %x0, [%x1, $9]", &[0xf8409020]),
        ("ldr %x0, [%sp, $16]", &[0xf9400be0]),
        ("str %x0, [%sp, $-16]!", &[0xf81f0fe0]),
        ("ldr %x0, [%sp], $16", &[0xf84107e0]),
        ("str %w0, [%x1, $4]!", &[0xb8004c20]),
        ("ldr %w0, [%x1], $-4", &[0xb85fc420]),
        ("ldrb %w0, [%x1, $1]", &[0x39400420]),
        ("ldrh %w0, [%x1, $2]", &[0x79400420]),
        ("strb %w0, [%x1]", &[0x39000020]),
        ("strh %w0, [%x1, $6]", &[0x79000c20]),
        ("ldrsb %x0, [%x1]", &[0x39800020]),
        ("ldrsb %w0, [%x1, $3]", &[0x39c00c20]),
        ("ldrsh %x0, [%x1, $2]", &[0x79800420]),
        ("ldrsh %w0, [%x1]", &[0x79c00020]),
        ("ldrsw %x0, [%x1, $4]", &[0xb9800420]),
        ("ldur %x0, [%x1, $-3]", &[0xf85fd020]),
        ("stur %w0, [%x1, $1]", &[0xb8001020]),
        ("ldurb %w0, [%x1, $-1]", &[0x385ff020]),
        ("ldr %x0, [%x1, %x2]", &[0xf8626820]),
        ("ldr %x0, [%x1, %x2, lsl $3]", &[0xf8627820]),
        ("ldr %w0, [%x1, %x2, lsl $2]", &[0xb8627820]),
        ("ldr %x0, [%x1, %w2, sxtw]", &[0xf862c820]),
        ("ldr %x0, [%x1, %w2, uxtw $3]", &[0xf8625820]),
        ("ldrb %w0, [%x1, %x2]", &[0x38626820]),
        ("str %x0, [%x1, %x2, lsl $3]", &[0xf8227820]),
        ("ldp %x29, %x30, [%sp]", &[0xa9407bfd]),
        ("ldp %x29, %x30, [%sp, $16]", &[0xa9417bfd]),
        ("stp %x29, %x30, [%sp, $-16]!", &[0xa9bf7bfd]),
        ("ldp %x29, %x30, [%sp], $16", &[0xa8c17bfd]),
        ("stp %w0, %w1, [%x2, $8]", &[0x29010440]),
        ("ldp %w0, %w1, [%x2], $-8", &[0x28ff0440]),
        ("stp %x0, %x1, [%x2, $504]", &[0xa91f8440]),
        ("ldp %x0, %x1, [%x2, $-512]", &[0xa9600440]),
        ("mov %x29, %sp", &[0x910003fd]),
        ("mov %fp, %lr", &[0xaa1e03fd]),
    ];

    #[test]
    fn encodings_match_gas() {
        for (src, expected) in CASES {
            assert_eq!(&words(src), expected, "{}", src);
        }
    }

    #[test]
    fn branches_within_a_section_are_resolved() {
        let src = "
start:
    b fwd
    nop
back:
    bl start
    b.ne back
    cbz %x0, fwd
    cbnz %w1, back
    tbz %x2, $3, fwd
    tbnz %x3, $40, back
    adr %x4, back
    ldr %x5, fwd
fwd:
    ret
";

        assert_eq!(
            words(src),
            [
                0x1400000a, 0xd503201f, 0x97fffffe, 0x54ffffe1, 0xb40000c0, 0x35ffffa1, 0x36180082,
                0xb747ff63, 0x10ffff44, 0x58000025, 0xd65f03c0
            ]
        );
    }

    #[test]
    fn invalid_sources_are_rejected() {
        let bad = [
            "add %x0, %w1, $1",          // レジスタ幅の不一致
            "add %x0, %x1, $4097",       // 即値が範囲外
            "mov %x0, $1, $2",           // オペランド数
            "mov %w0, $0x100000000",     // 32bit に収まらない
            "and %x0, %x1, $0",          // ビットマスクで表せない
            "ldr %x0, [%x1, $40000]",    // オフセットが範囲外
            "ldr %w0, [%x1, %w2]",       // Wのindexには拡張が必要
            "ldrb %x0, [%x1]",           // ldrb は %w
            "ldp %x0, %x1, [%sp, $4]",   // 境界が合わない
            "tbz %w0, $40, l\nl:",       // %w にはbit 32以上は使えない
            "b nowhere",                 // 未定義シンボル
            "foo %x0",                   // 未対応の命令
            "mov %q0, $1",               // 不明なレジスタ
            "b.xx l\nl:",                // 不明な条件
            ".data\nnop",                // .data に命令
            ".bogus 1",                  // 未対応のディレクティブ
            "l:\nl:",                    // ラベルの重複
            ".text\n.byte 1\nnop",       // 命令が4バイト境界にない
            "add %x0, %x1, %x2, ror $1", // add に ror は使えない
            "sub %x0, %x1, :lo12:sym",   // :lo12: は add だけ
        ];

        for src in bad {
            assert!(assemble_aarch64(src).is_err(), "should fail: {}", src);
        }
    }

    #[test]
    fn data_directives() {
        let (text, data) = assemble_aarch64(
            ".data
a: .byte 1, 2
.align 2
b: .word 5
.hword 0x1234
.asciz \"ab\"
.balign 8
.quad -1
.float 1.5
.zero 2
",
        )
        .unwrap();

        assert!(text.is_empty());
        assert_eq!(
            data,
            [
                1, 2, 0, 0, // .byte + .align 2 (4バイト境界)
                5, 0, 0, 0, // .word
                0x34, 0x12, // .hword
                b'a', b'b', 0, // .asciz
                0, 0, 0, // .balign 8
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // .quad
                0x00, 0x00, 0xc0, 0x3f, // .float 1.5
                0, 0, // .zero
            ]
        );
    }

    #[test]
    fn text_alignment_is_padded_with_nop() {
        assert_eq!(words("nop\n.align 4\nnop"), [0xd503201f; 5]);
    }

    #[test]
    fn comments_and_separators() {
        let src = "nop // a\n/* b\n c */ nop # d\nnop; nop";
        assert_eq!(words(src), [0xd503201f; 4]);
    }
}
