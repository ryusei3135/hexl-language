//! アセンブラのディレクティブ (`.text` / `.quad` など) の対応表
//!
//! # ディレクティブを追加するには (このファイルだけ編集すればよい)
//!
//! `emit_directive` の `match` に1行(または1ブロック)足す。
//!
//! - 別名を足すだけなら、既存の行に `| ".別名"` を足す。
//! - 引数の取り出しには下部の補助関数を使う。
//!   `symbol_args` (シンボル列) / `data_values` (数値かシンボル列) /
//!   `float_values` (小数または整数) / `string_values` (文字列列) /
//!   `single_int_arg` (数値1つ)
//! - バッファへの書き込みは `Emitter` の `current_buffer()` / `add_data_fixup()` を使う。
//!
//! 未対応のディレクティブは `unsupported directive: ...` のエラーになる。
//!
//! # gas との対応
//!
//! aarch64 の gas と同じく、`.align N` / `.p2align N` は **2^N バイト** 境界、
//! `.balign N` は **N バイト** 境界に揃える (x64 版の `.align` はバイト数なので注意)。

use super::emitter::{Emitter, FixupKind, Section};
use super::parse::Operand;

const NOP: u32 = 0xd503_201f;

pub(super) fn emit_directive(
    e: &mut Emitter,
    name: &str,
    operands: &[Operand],
) -> Result<(), String> {
    match name {
        ".text" => e.current_section = Section::Text,
        ".data" | ".rodata" => e.current_section = Section::Data,

        ".section" => {
            let names = symbol_args(name, operands)?;

            match names.as_slice() {
                [n] => match n.as_str() {
                    ".text" | "text" => e.current_section = Section::Text,
                    ".data" | "data" | ".rodata" | "rodata" => e.current_section = Section::Data,
                    other => return Err(format!("unsupported section: {}", other)),
                },
                _ => return Err("`.section` requires exactly one section name".into()),
            }
        }

        ".global" | ".globl" => {
            for n in symbol_args(name, operands)? {
                if !e.globals.contains(&n) {
                    e.globals.push(n);
                }
            }
        }

        ".extern" => {
            for n in symbol_args(name, operands)? {
                if !e.externs.contains(&n) {
                    e.externs.push(n);
                }
            }
        }

        // 整数データ定義
        ".byte" => emit_data(e, name, operands, 1)?,
        ".hword" | ".short" | ".2byte" => emit_data(e, name, operands, 2)?,
        ".word" | ".long" | ".int" | ".4byte" => emit_data(e, name, operands, 4)?,
        ".quad" | ".xword" | ".dword" | ".8byte" => emit_data(e, name, operands, 8)?,

        // 小数データ定義
        ".float" | ".single" => emit_float(e, name, operands, 4)?,
        ".double" => emit_float(e, name, operands, 8)?,

        // 文字列
        ".ascii" => emit_string(e, name, operands, false)?,
        ".asciz" | ".string" => emit_string(e, name, operands, true)?,

        // アライン / パディング
        ".align" | ".p2align" => {
            let n = single_int_arg(name, operands)?;

            if !(0..=16).contains(&n) {
                return Err(format!("`{}` requires a value between 0 and 16", name));
            }

            emit_align(e, 1usize << n);
        }
        ".balign" => {
            let n = single_int_arg(name, operands)?;

            if n <= 0 {
                return Err(format!("`{}` requires a positive value", name));
            }

            emit_align(e, n as usize);
        }
        ".zero" | ".skip" | ".space" => {
            let n = single_int_arg(name, operands)?;

            if n < 0 {
                return Err(format!("`{}` requires a non-negative value", name));
            }

            e.current_buffer().extend(std::iter::repeat(0u8).take(n as usize));
        }

        other => return Err(format!("unsupported directive: {}", other)),
    }

    Ok(())
}

// ============================================================
// 出力処理
// ============================================================

fn emit_align(e: &mut Emitter, align: usize) {
    let is_text = e.current_section == Section::Text;
    let buffer = e.current_buffer();

    while buffer.len() % align != 0 {
        // .text は NOP 命令で埋める (4バイト単位で埋まる間だけ)
        if is_text && buffer.len() % 4 == 0 && align - buffer.len() % align >= 4 {
            buffer.extend_from_slice(&NOP.to_le_bytes());
        } else {
            buffer.push(0);
        }
    }
}

fn emit_data(e: &mut Emitter, name: &str, operands: &[Operand], size: usize) -> Result<(), String> {
    if operands.is_empty() {
        return Err(format!("`{}` requires at least one value", name));
    }

    for operand in operands {
        match operand {
            Operand::Immediate(v) => {
                let bytes = v.to_le_bytes();
                e.current_buffer().extend_from_slice(&bytes[..size]);
            }

            Operand::Symbol(sym) => {
                let kind = match size {
                    4 => FixupKind::Abs32,
                    8 => FixupKind::Abs64,
                    _ => return Err(format!("`{}` cannot hold a symbol address", name)),
                };

                e.add_data_fixup(sym, kind);
                e.current_buffer().extend(std::iter::repeat(0u8).take(size));
            }

            Operand::Str(s) if size == 1 => {
                e.current_buffer().extend_from_slice(s.as_bytes());
            }

            other => return Err(format!("invalid operand for `{}`: {:?}", name, other)),
        }
    }

    Ok(())
}

fn emit_float(e: &mut Emitter, name: &str, operands: &[Operand], size: usize) -> Result<(), String> {
    for v in float_values(name, operands)? {
        if size == 4 {
            e.current_buffer().extend_from_slice(&(v as f32).to_le_bytes());
        } else {
            e.current_buffer().extend_from_slice(&v.to_le_bytes());
        }
    }

    Ok(())
}

fn emit_string(
    e: &mut Emitter,
    name: &str,
    operands: &[Operand],
    nul: bool,
) -> Result<(), String> {
    for s in string_values(name, operands)? {
        e.current_buffer().extend_from_slice(s.as_bytes());

        if nul {
            e.current_buffer().push(0);
        }
    }

    Ok(())
}

// ============================================================
// 引数の取り出し
// ============================================================

fn symbol_args(name: &str, operands: &[Operand]) -> Result<Vec<String>, String> {
    if operands.is_empty() {
        return Err(format!("`{}` requires at least one symbol", name));
    }

    operands
        .iter()
        .map(|o| match o {
            Operand::Symbol(s) => Ok(s.clone()),
            other => Err(format!("`{}` expects a symbol, got {:?}", name, other)),
        })
        .collect()
}

#[allow(dead_code)]
fn data_values(name: &str, operands: &[Operand]) -> Result<Vec<Operand>, String> {
    operands
        .iter()
        .map(|o| match o {
            Operand::Immediate(_) | Operand::Symbol(_) => Ok(o.clone()),
            other => Err(format!("invalid operand for `{}`: {:?}", name, other)),
        })
        .collect()
}

fn float_values(name: &str, operands: &[Operand]) -> Result<Vec<f64>, String> {
    if operands.is_empty() {
        return Err(format!("`{}` requires at least one value", name));
    }

    operands
        .iter()
        .map(|o| match o {
            Operand::Float(f) => Ok(*f),
            Operand::Immediate(i) => Ok(*i as f64),
            other => Err(format!("`{}` expects a number, got {:?}", name, other)),
        })
        .collect()
}

fn string_values(name: &str, operands: &[Operand]) -> Result<Vec<String>, String> {
    if operands.is_empty() {
        return Err(format!("`{}` requires at least one string", name));
    }

    operands
        .iter()
        .map(|o| match o {
            Operand::Str(s) => Ok(s.clone()),
            other => Err(format!("`{}` expects a string, got {:?}", name, other)),
        })
        .collect()
}

fn single_int_arg(name: &str, operands: &[Operand]) -> Result<i64, String> {
    match operands {
        [Operand::Immediate(n)] => Ok(*n),
        _ => Err(format!("`{}` requires exactly one integer", name)),
    }
}
