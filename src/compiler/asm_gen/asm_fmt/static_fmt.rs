//! x64/aarch64の静的フォーマット(`x64.rs`/`aarch64.rs`)で共通の型
//!
//! `String` / `Vec` / `HashMap` は `static` で直接初期化できないため、
//! `&'static str` とスライスで同じ構造を持つ型を用意している。
//! 既存の `asm_setting::AsmFormat` が必要な場合は
//! `to_asm_format()` で変換できる。

use std::collections::HashMap;

use crate::asm_setting::{
    AsmFormat, DataFmt, Func, FuncArgsReg, OpSizeFmt, OperandInfo, Reg, ValueFmt,
};

pub struct StaticReg {
    pub db: &'static [&'static str],
    pub dw: &'static [&'static str],
    pub dd: &'static [&'static str],
    pub dq: &'static [&'static str],
    pub xmm: &'static [&'static str],
    pub ymm: &'static [&'static str],
    pub zmm: &'static [&'static str],
}

pub struct StaticOperandInfo {
    pub len: usize,
    pub template: &'static str,
}

pub struct StaticFunc {
    pub extern_def: &'static str,
    pub ret: usize,
    pub call: &'static str,
}

pub struct StaticFuncArgsReg {
    /// (OS名, レジスタ番号の並び)
    pub fmt: &'static [(&'static str, &'static [usize])],
}

pub struct StaticDataFmt {
    pub head: &'static str,
    pub fmt: &'static str,
}

pub struct StaticOpSizeFmt {
    pub db: &'static str,
    pub dw: &'static str,
    pub dd: &'static str,
    pub dq: &'static str,
}

pub struct StaticValueFmt {
    pub reg: &'static str,
    pub num: &'static str,
    pub static_var: &'static str,
    pub string: &'static str,
    pub global: &'static str,
    pub data: StaticDataFmt,
    pub op_size: StaticOpSizeFmt,
    pub mnemonic_size: bool,
    pub ref_stack: &'static str,
    pub frame: &'static str,
    pub frame_end: &'static str,
}

pub struct StaticAsmFormat {
    pub reg: StaticReg,
    pub args: StaticFuncArgsReg,
    pub section: &'static str,
    pub fmt: StaticValueFmt,
    /// (命令名, 情報)
    pub op: &'static [(&'static str, StaticOperandInfo)],
    pub func: StaticFunc,
}

fn to_strings(src: &[&str]) -> Vec<String> {
    src.iter().map(|s| s.to_string()).collect()
}

impl StaticAsmFormat {
    /// 既存の `AsmFormat`(`AsmEmitter` が受け取る型)へ変換する
    pub fn to_asm_format(&self) -> AsmFormat {
        AsmFormat {
            reg: Reg {
                db: to_strings(self.reg.db),
                dw: to_strings(self.reg.dw),
                dd: to_strings(self.reg.dd),
                dq: to_strings(self.reg.dq),
                xmm: to_strings(self.reg.xmm),
                ymm: to_strings(self.reg.ymm),
                zmm: to_strings(self.reg.zmm),
            },
            args: FuncArgsReg {
                fmt: self
                    .args
                    .fmt
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_vec()))
                    .collect::<HashMap<_, _>>(),
            },
            section: self.section.to_string(),
            fmt: ValueFmt {
                reg: self.fmt.reg.to_string(),
                num: self.fmt.num.to_string(),
                static_var: self.fmt.static_var.to_string(),
                string: self.fmt.string.to_string(),
                global: self.fmt.global.to_string(),
                data: DataFmt {
                    head: self.fmt.data.head.to_string(),
                    fmt: self.fmt.data.fmt.to_string(),
                },
                op_size: OpSizeFmt {
                    db: self.fmt.op_size.db.to_string(),
                    dw: self.fmt.op_size.dw.to_string(),
                    dd: self.fmt.op_size.dd.to_string(),
                    dq: self.fmt.op_size.dq.to_string(),
                },
                mnemonic_size: self.fmt.mnemonic_size,
                ref_stack: self.fmt.ref_stack.to_string(),
                frame: self.fmt.frame.to_string(),
                frame_end: self.fmt.frame_end.to_string(),
            },
            op: self
                .op
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        OperandInfo {
                            len: v.len,
                            template: v.template.to_string(),
                        },
                    )
                })
                .collect::<HashMap<_, _>>(),
            func: Func {
                extern_def: self.func.extern_def.to_string(),
                ret: self.func.ret,
                call: self.func.call.to_string(),
            },
        }
    }
}
