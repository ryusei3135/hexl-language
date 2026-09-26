use super::emitter::{self, DataValue, Directive, Reg};
use super::parse;

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

fn convert_directive(
    name: &str,
    operands: &[parse::Operand],
) -> Result<Vec<emitter::Statement>, String> {
    match name {
        ".text" => Ok(vec![directive(Directive::Text)]),

        ".data" => Ok(vec![directive(Directive::Data)]),

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

        // 文字列データ定義
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

fn parse_register(name: &str) -> Result<Reg, String> {
    use Reg::*;

    Ok(match name {
        // 8 bit
        "%al" => Al,
        "%cl" => Cl,
        "%dl" => Dl,
        "%bl" => Bl,
        "%spl" => Spl,
        "%bpl" => Bpl,
        "%sil" => Sil,
        "%dil" => Dil,
        "%r8b" => R8b,
        "%r9b" => R9b,
        "%r10b" => R10b,
        "%r11b" => R11b,
        "%r12b" => R12b,
        "%r13b" => R13b,
        "%r14b" => R14b,
        "%r15b" => R15b,

        // 16 bit
        "%ax" => Ax,
        "%cx" => Cx,
        "%dx" => Dx,
        "%bx" => Bx,
        "%sp" => Sp,
        "%bp" => Bp,
        "%si" => Si,
        "%di" => Di,
        "%r8w" => R8w,
        "%r9w" => R9w,
        "%r10w" => R10w,
        "%r11w" => R11w,
        "%r12w" => R12w,
        "%r13w" => R13w,
        "%r14w" => R14w,
        "%r15w" => R15w,

        // 32 bit
        "%eax" => Eax,
        "%ecx" => Ecx,
        "%edx" => Edx,
        "%ebx" => Ebx,
        "%esp" => Esp,
        "%ebp" => Ebp,
        "%esi" => Esi,
        "%edi" => Edi,
        "%r8d" => R8d,
        "%r9d" => R9d,
        "%r10d" => R10d,
        "%r11d" => R11d,
        "%r12d" => R12d,
        "%r13d" => R13d,
        "%r14d" => R14d,
        "%r15d" => R15d,

        // 64 bit
        "%rax" => Rax,
        "%rcx" => Rcx,
        "%rdx" => Rdx,
        "%rbx" => Rbx,
        "%rsp" => Rsp,
        "%rbp" => Rbp,
        "%rsi" => Rsi,
        "%rdi" => Rdi,
        "%r8" => R8,
        "%r9" => R9,
        "%r10" => R10,
        "%r11" => R11,
        "%r12" => R12,
        "%r13" => R13,
        "%r14" => R14,
        "%r15" => R15,

        "%rip" => Rip,

        other => {
            return Err(format!("unknown register: {}", other));
        }
    })
}
