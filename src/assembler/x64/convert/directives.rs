//! アセンブラのディレクティブ (`.text` / `.quad` など) の対応表
//!
//! # ディレクティブを追加するには (このファイルだけ編集すればよい)
//!
//! `convert_directive` の `match` に1行(または1ブロック)足す。
//!
//! - **既存の出力処理で表現できる場合** (データ定義・アライン・0埋めなど) は
//!   このファイルだけで完結する。
//!
//!   ```text
//!   // 別名を足す例: `.hword` を `.word` と同じ扱いにする
//!   ".word" | ".short" | ".2byte" | ".hword" => ...
//!   ```
//!
//! - 引数の取り出しには下部の補助関数を使う。
//!   `symbol_args` (シンボル列) / `integer_values` (数値かシンボル列) /
//!   `string_values` (文字列列) / `single_int_arg` (数値1つ)
//!
//! - **新しい出力動作が必要な場合** は、上に加えて
//!   `emitter.rs` の `enum Directive` に variant を足し、`emit_directive`
//!   に処理を書く (2ファイル)。
//!
//! 未対応のディレクティブは `unsupported directive: ...` のエラーになる。

use super::super::emitter::{self, DataValue, Directive};
use super::super::parse;

pub(super) fn convert_directive(
    name: &str,
    operands: &[parse::Operand],
) -> Result<Vec<emitter::Statement>, String> {
    match name {
        ".text" => Ok(vec![directive(Directive::Text)]),
        ".data" => Ok(vec![directive(Directive::Data)]),
        // `.section test` / `.section .test` / `.test`
        ".test" => Ok(vec![directive(Directive::Test)]),
        ".section" => {
            let names = symbol_args(name, operands)?;

            match names.as_slice() {
                [n] => match n.as_str() {
                    "test" | ".test" => Ok(vec![directive(Directive::Test)]),
                    ".text" => Ok(vec![directive(Directive::Text)]),
                    ".data" => Ok(vec![directive(Directive::Data)]),
                    other => Err(format!("unsupported section: {}", other)),
                },
                _ => Err("`.section` requires exactly one section name".into()),
            }
        }
        ".global" | ".globl" => {
            let names = symbol_args(name, operands)?;

            Ok(names
                .into_iter()
                .map(|name| directive(Directive::Global(name)))
                .collect())
        }
        ".extern" => {
            let names = symbol_args(name, operands)?;

            Ok(names
                .into_iter()
                .map(|name| directive(Directive::Extern(name)))
                .collect())
        }
        // 整数データ定義
        ".byte" => Ok(vec![directive(Directive::Db(integer_values(
            name, operands,
        )?))]),

        ".word" | ".short" | ".2byte" => Ok(vec![directive(Directive::Dw(integer_values(
            name, operands,
        )?))]),

        ".long" | ".int" | ".4byte" => Ok(vec![directive(Directive::Dd(integer_values(
            name, operands,
        )?))]),

        ".quad" | ".8byte" => Ok(vec![directive(Directive::Dq(integer_values(
            name, operands,
        )?))]),
        ".ascii" => Ok(vec![directive(Directive::Db(string_values(
            name, operands, false,
        )?))]),
        ".asciz" | ".string" => Ok(vec![directive(Directive::Db(string_values(
            name, operands, true,
        )?))]),
        // アライン / パディング
        ".align" | ".balign" => {
            let n = single_int_arg(name, operands)?;

            if n <= 0 {
                return Err(format!("`{}` requires a positive value", name));
            }
            Ok(vec![directive(Directive::Align(n as usize))])
        }
        ".zero" | ".skip" | ".space" => {
            let n = single_int_arg(name, operands)?;

            if n < 0 {
                return Err(format!("`{}` requires a non-negative value", name));
            }

            Ok(vec![directive(Directive::Zero(n as usize))])
        }
        other => Err(format!("unsupported directive: {}", other)),
    }
}

// ============================================================
// Directive argument helpers
// ============================================================

fn symbol_args(directive: &str, operands: &[parse::Operand]) -> Result<Vec<String>, String> {
    if operands.is_empty() {
        return Err(format!("`{}` requires at least one symbol", directive));
    }

    operands
        .iter()
        .map(|operand| match operand {
            parse::Operand::Symbol(name) => Ok(name.clone()),

            other => Err(format!(
                "`{}`: invalid argument (expected symbol): {:?}",
                directive, other
            )),
        })
        .collect()
}

fn integer_values(
    directive: &str,
    operands: &[parse::Operand],
) -> Result<Vec<DataValue>, String> {
    if operands.is_empty() {
        return Err(format!("`{}` requires at least one value", directive));
    }

    operands
        .iter()
        .map(|operand| match operand {
            parse::Operand::Immediate(value) => Ok(DataValue::Integer(*value)),

            parse::Operand::Symbol(name) => Ok(DataValue::Symbol(name.clone())),

            other => Err(format!(
                "`{}`: invalid argument (expected number or symbol): {:?}",
                directive, other
            )),
        })
        .collect()
}

fn string_values(
    directive: &str,
    operands: &[parse::Operand],
    nul_terminate: bool,
) -> Result<Vec<DataValue>, String> {
    if operands.is_empty() {
        return Err(format!("`{}` requires at least one string", directive));
    }

    operands
        .iter()
        .map(|operand| match operand {
            parse::Operand::Str(value) => {
                let mut bytes = value.as_bytes().to_vec();

                if nul_terminate {
                    bytes.push(0);
                }

                Ok(DataValue::String(bytes))
            }

            other => Err(format!(
                "`{}`: invalid argument (expected string literal): {:?}",
                directive, other
            )),
        })
        .collect()
}

fn single_int_arg(directive: &str, operands: &[parse::Operand]) -> Result<i64, String> {
    match operands {
        [parse::Operand::Immediate(value)] => Ok(*value),

        [] => Err(format!("`{}` requires a numeric argument", directive)),

        _ => Err(format!(
            "`{}` requires exactly one numeric argument",
            directive
        )),
    }
}

fn directive(directive: Directive) -> emitter::Statement {
    emitter::Statement::Directive(directive)
}
