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
//!   `float_values` (小数または整数) /
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
        // 小数データ定義
        ".float" | ".single" => Ok(vec![directive(Directive::Float(float_values(
            name, operands,
        )?))]),

        ".double" => Ok(vec![directive(Directive::Double(float_values(
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

fn integer_values(directive: &str, operands: &[parse::Operand]) -> Result<Vec<DataValue>, String> {
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

// `.float 1.1` のような小数、または `.float 1` のような整数を f64 として取り出す
fn float_values(directive: &str, operands: &[parse::Operand]) -> Result<Vec<f64>, String> {
    if operands.is_empty() {
        return Err(format!("`{}` requires at least one value", directive));
    }

    operands
        .iter()
        .map(|operand| match operand {
            parse::Operand::Float(value) => Ok(*value),

            parse::Operand::Immediate(value) => Ok(*value as f64),

            other => Err(format!(
                "`{}`: invalid argument (expected number): {:?}",
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

#[cfg(test)]
mod float_tests {
    use crate::assembler::x64::emitter::Emitter;
    use crate::assembler::x64::{Lexer, Parser, convert};

    fn assemble_data(source: &str) -> Result<(Vec<u8>, Emitter), String> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::new(tokens).parse()?;
        let program = convert::convert_program(&ast)?;
        let mut emitter = Emitter::new();
        emitter.emit_program(&program)?;
        Ok((emitter.data.clone(), emitter))
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// 期待値は GNU as でアセンブルした `.data` の出力
    #[test]
    fn float_directive_matches_gnu_as() {
        let source = "\
.data
.align 4
M0: .float 1.1
.align 4
M1: .float 2.1
.align 4
.double 1.1, -2.5
.float 3, -0.5, 1e3, 2.5e-2
";
        let (data, emitter) = assemble_data(source).unwrap();

        assert_eq!(
            hex(&data),
            "cdcc8c3f666606409a9999999999f13f00000000000004c000004040000000bf00007a44cdcccc3c"
        );
        assert_eq!(emitter.data_labels["M0"], 0);
        assert_eq!(emitter.data_labels["M1"], 4);
    }

    #[test]
    fn float_literal_is_rejected_as_instruction_operand() {
        assert!(assemble_data(".text\nmovss $1.5, %xmm0\n").is_err());
    }

    #[test]
    fn float_directive_rejected_in_text_section() {
        assert!(assemble_data(".text\n.float 1.1\n").is_err());
    }
}
