use super::emitter::{self, Reg};
use super::parse;

// ディレクティブ名 -> 処理の対応表は `convert/directives.rs`
mod directives;
use directives::convert_directive;

// ============================================================
// Program / Statement
// ============================================================

pub fn convert_program(ast: &parse::Program) -> Result<emitter::Program, String> {
    let mut statements = Vec::new();

    for statement in &ast.statements {
        statements.extend(convert_statement(statement)?);
    }

    Ok(emitter::Program { statements })
}

// 1つの parse::Statement が複数の emitter::Statement に展開されることがある
// (例: `.globl foo, bar` はシンボル毎に1つの Directive になる)
fn convert_statement(statement: &parse::Statement) -> Result<Vec<emitter::Statement>, String> {
    match statement {
        parse::Statement::Label(name) => Ok(vec![emitter::Statement::Label(name.clone())]),

        parse::Statement::Directive(name, args) => {
            // (parse.rs のパーサ自身はこの variant を作らないが、
            //  互換性のため文字列引数を Symbol 相当として扱う)
            let operands = args
                .iter()
                .map(|s| parse::Operand::Symbol(s.clone()))
                .collect::<Vec<_>>();

            convert_directive(name, &operands)
        }

        parse::Statement::Instruction(inst) => {
            // parse.rs は `.text` / `.globl foo` のようなディレクティブも
            // 先頭が `.` の Ident を mnemonic とする Instruction として
            // 解析するので、ここで振り分ける。
            if inst.mnemonic.starts_with('.') {
                convert_directive(&inst.mnemonic, &inst.operands)
            } else {
                Ok(vec![emitter::Statement::Instruction(convert_instruction(
                    inst,
                )?)])
            }
        }
    }
}

// ============================================================
// Instruction
// ============================================================

fn convert_instruction(inst: &parse::Instruction) -> Result<emitter::Instruction, String> {
    let operands = inst
        .operands
        .iter()
        .map(convert_operand)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(emitter::Instruction {
        mnemonic: inst.mnemonic.to_ascii_lowercase(),
        operands,
    })
}

// ============================================================
// Operand
// ============================================================

fn convert_operand(operand: &parse::Operand) -> Result<emitter::Operand, String> {
    match operand {
        parse::Operand::Register(name) => Ok(emitter::Operand::Register(parse_register(name)?)),

        parse::Operand::Immediate(value) => Ok(emitter::Operand::Immediate(*value)),

        parse::Operand::Symbol(name) => Ok(emitter::Operand::Symbol(name.clone())),

        parse::Operand::Memory(mem) => Ok(emitter::Operand::Memory(convert_memory(mem)?)),

        parse::Operand::Str(value) => Err(format!(
            "string literal is not valid as an instruction operand: {:?}",
            value
        )),
    }
}

fn convert_memory(mem: &parse::MemoryOperand) -> Result<emitter::MemoryOperand, String> {
    let base = mem.base.as_deref().map(parse_register).transpose()?;

    let index = mem.index.as_deref().map(parse_register).transpose()?;

    Ok(emitter::MemoryOperand {
        displacement: mem.displacement,
        base,
        index,
        scale: mem.scale,
        symbol: mem.symbol.clone(),
    })
}

// ============================================================
// Register
// ============================================================

// レジスタ名の対応表は `reg.rs` にある (追加はそちらで行う)
fn parse_register(name: &str) -> Result<Reg, String> {
    Reg::from_name(name).ok_or_else(|| format!("unknown register: {}", name))
}
